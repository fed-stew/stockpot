//! Bridge mode for external UI communication.
//!
//! Provides NDJSON-based protocol for UI integrations (VS Code, web UI, etc.).
//! All messages are JSON objects separated by newlines.
//!
//! ## Protocol
//!
//! ### Outbound Messages (stdout)
//! ```json
//! {"type": "ready", "version": "0.5.0"}
//! {"type": "text_delta", "text": "Hello "}
//! {"type": "tool_call", "name": "read_file", "args": {...}}
//! {"type": "tool_result", "name": "read_file", "success": true, "output": "..."}
//! {"type": "complete", "run_id": "..."}
//! {"type": "error", "message": "..."}
//! ```
//!
//! ### Inbound Commands (stdin)
//! ```json
//! {"type": "prompt", "text": "...", "agent": "stockpot"}
//! {"type": "cancel"}
//! {"type": "tool_response", "name": "...", "approved": true}
//! ```

use crate::agents::{AgentExecutor, AgentManager, StreamEvent};
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{AgentEvent, Message, MessageBus, MessageReceiver, ToolStatus};
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tokio::sync::mpsc;
use tracing::debug;

/// Outbound message types sent to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeOutMessage {
    /// Bridge is ready to receive commands.
    Ready {
        version: String,
        agent: String,
        model: String,
    },
    /// Text content streaming.
    TextDelta { text: String },
    /// Thinking/reasoning content.
    ThinkingDelta { text: String },
    /// Tool call started.
    ToolCallStart {
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
    },
    /// Tool call arguments streaming.
    ToolCallDelta { delta: String },
    /// Tool call completed.
    ToolCallComplete { tool_name: String },
    /// Tool executed with result.
    ToolExecuted {
        tool_name: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Request step started.
    RequestStart { step: u32 },
    /// Run completed successfully.
    Complete { run_id: String },
    /// Error occurred.
    Error { message: String },
    /// Agent switched.
    AgentChanged { agent: String },
    /// Model switched.
    ModelChanged { model: String },
    /// MCP server status.
    McpStatus { servers: Vec<McpServerStatus> },
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatus {
    pub name: String,
    pub running: bool,
}

impl BridgeOutMessage {
    /// Convert a Message bus event to a Bridge output message.
    fn from_message(msg: Message) -> Option<Self> {
        match msg {
            Message::TextDelta(delta) => Some(BridgeOutMessage::TextDelta { text: delta.text }),
            Message::Thinking(thinking) => Some(BridgeOutMessage::ThinkingDelta {
                text: thinking.text,
            }),
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => None, // Silent - UI can track from tool/text events
                AgentEvent::Completed { run_id } => Some(BridgeOutMessage::Complete { run_id }),
                AgentEvent::Error { message } => Some(BridgeOutMessage::Error { message }),
            },
            Message::Tool(tool) => match tool.status {
                ToolStatus::Started => Some(BridgeOutMessage::ToolCallStart {
                    tool_name: tool.tool_name,
                    tool_call_id: None,
                }),
                ToolStatus::ArgsStreaming => None, // We don't have delta here
                ToolStatus::Executing => Some(BridgeOutMessage::ToolCallComplete {
                    tool_name: tool.tool_name,
                }),
                ToolStatus::Completed => Some(BridgeOutMessage::ToolExecuted {
                    tool_name: tool.tool_name,
                    success: true,
                    error: None,
                }),
                ToolStatus::Failed => Some(BridgeOutMessage::ToolExecuted {
                    tool_name: tool.tool_name,
                    success: false,
                    error: tool.error,
                }),
            },
            // Other message types don't map to bridge protocol
            _ => None,
        }
    }
}

/// Inbound command types from the UI.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeInCommand {
    /// Send a prompt to the agent.
    Prompt {
        text: String,
        #[serde(default)]
        agent: Option<String>,
        #[serde(default)]
        model: Option<String>,
    },
    /// Cancel the current operation.
    Cancel,
    /// Switch agent.
    SwitchAgent { agent: String },
    /// Switch model.
    SwitchModel { model: String },
    /// Start MCP server.
    McpStart {
        #[serde(default)]
        name: Option<String>,
    },
    /// Stop MCP server.
    McpStop {
        #[serde(default)]
        name: Option<String>,
    },
    /// List MCP tools.
    McpList,
    /// Approve/reject a tool call (for confirmation mode).
    ToolResponse { call_id: String, approved: bool },
    /// Shutdown the bridge.
    Shutdown,
}

/// Bridge renderer that outputs NDJSON to stdout.
struct BridgeRenderer;

impl BridgeRenderer {
    /// Run the render loop, converting Messages to NDJSON output.
    async fn run_loop(&self, mut receiver: MessageReceiver) {
        while let Ok(message) = receiver.recv().await {
            if let Some(bridge_msg) = BridgeOutMessage::from_message(message) {
                if let Ok(json) = serde_json::to_string(&bridge_msg) {
                    println!("{}", json);
                }
            }
        }
    }
}

/// Bridge state.
struct Bridge<'a> {
    db: &'a Database,
    agents: AgentManager,
    tool_registry: SpotToolRegistry,
    mcp_manager: McpManager,
    model_registry: ModelRegistry,
    current_model: String,
    /// Message bus for event-driven output.
    message_bus: MessageBus,
}

impl<'a> Bridge<'a> {
    fn new(db: &'a Database) -> Self {
        let settings = Settings::new(db);
        let current_model = settings.model();

        Self {
            db,
            agents: AgentManager::new(),
            tool_registry: SpotToolRegistry::new(),
            mcp_manager: McpManager::new(),
            model_registry: ModelRegistry::load_from_db(db).unwrap_or_default(),
            current_model,
            message_bus: MessageBus::new(),
        }
    }

    /// Send a message to the UI.
    fn emit(&self, msg: BridgeOutMessage) {
        if let Ok(json) = serde_json::to_string(&msg) {
            println!("{}", json);
        }
    }

    /// Handle a prompt command.
    async fn handle_prompt(&mut self, text: &str) {
        self.handle_prompt_with_bus(text).await;
    }

    /// Handle a prompt using the message bus architecture.
    ///
    /// This approach uses the message bus for event-driven output,
    /// making sub-agent output visible through the same NDJSON protocol.
    async fn handle_prompt_with_bus(&mut self, text: &str) {
        let agent = match self.agents.current() {
            Some(a) => a,
            None => {
                self.emit(BridgeOutMessage::Error {
                    message: "No agent selected".to_string(),
                });
                return;
            }
        };

        debug!(agent = %agent.name(), model = %self.current_model, "Bridge handling prompt");

        // Create executor with message bus
        let executor =
            AgentExecutor::new(self.db, &self.model_registry).with_bus(self.message_bus.sender());

        // Spawn bridge renderer
        let receiver = self.message_bus.subscribe();
        let render_handle = tokio::spawn(async move {
            BridgeRenderer.run_loop(receiver).await;
        });

        // Execute - all events flow through the bus automatically!
        let result = executor
            .execute_with_bus(
                agent,
                &self.current_model,
                text,
                None, // No history in bridge mode for now
                &self.tool_registry,
                &self.mcp_manager,
            )
            .await;

        // Give renderer a moment to finish processing, then abort if needed
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        render_handle.abort();

        // Handle errors (successes are emitted through the bus)
        if let Err(e) = result {
            self.emit(BridgeOutMessage::Error {
                message: e.to_string(),
            });
        }
    }

    /// Legacy prompt handling (direct stream processing).
    ///
    /// Kept for reference and fallback if needed.
    #[allow(dead_code)]
    async fn handle_prompt_legacy(&mut self, text: &str) {
        let agent = match self.agents.current() {
            Some(a) => a,
            None => {
                self.emit(BridgeOutMessage::Error {
                    message: "No agent selected".to_string(),
                });
                return;
            }
        };

        let executor = AgentExecutor::new(self.db, &self.model_registry);

        match executor
            .execute_stream(
                agent,
                &self.current_model,
                text,
                None,
                &self.tool_registry,
                &self.mcp_manager,
            )
            .await
        {
            Ok(mut stream) => {
                while let Some(event_result) = stream.recv().await {
                    match event_result {
                        Ok(event) => {
                            let msg = match event {
                                StreamEvent::RunStart { run_id: _ } => {
                                    Some(BridgeOutMessage::Ready {
                                        version: env!("CARGO_PKG_VERSION").to_string(),
                                        agent: self.agents.current_name(),
                                        model: self.current_model.clone(),
                                    })
                                }
                                StreamEvent::RequestStart { step } => {
                                    Some(BridgeOutMessage::RequestStart { step })
                                }
                                StreamEvent::TextDelta { text } => {
                                    Some(BridgeOutMessage::TextDelta { text })
                                }
                                StreamEvent::ThinkingDelta { text } => {
                                    Some(BridgeOutMessage::ThinkingDelta { text })
                                }
                                StreamEvent::ToolCallStart {
                                    tool_name,
                                    tool_call_id,
                                } => Some(BridgeOutMessage::ToolCallStart {
                                    tool_name,
                                    tool_call_id,
                                }),
                                StreamEvent::ToolCallDelta { delta } => {
                                    Some(BridgeOutMessage::ToolCallDelta { delta })
                                }
                                StreamEvent::ToolCallComplete { tool_name } => {
                                    Some(BridgeOutMessage::ToolCallComplete { tool_name })
                                }
                                StreamEvent::ToolExecuted {
                                    tool_name,
                                    success,
                                    error,
                                } => Some(BridgeOutMessage::ToolExecuted {
                                    tool_name,
                                    success,
                                    error,
                                }),
                                StreamEvent::ResponseComplete { .. } => None,
                                StreamEvent::OutputReady => None,
                                StreamEvent::RunComplete { run_id, .. } => {
                                    Some(BridgeOutMessage::Complete { run_id })
                                }
                                StreamEvent::Error { message } => {
                                    Some(BridgeOutMessage::Error { message })
                                }
                            };
                            if let Some(m) = msg {
                                self.emit(m);
                            }
                        }
                        Err(e) => {
                            self.emit(BridgeOutMessage::Error {
                                message: e.to_string(),
                            });
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                self.emit(BridgeOutMessage::Error {
                    message: e.to_string(),
                });
            }
        }
    }

    /// Handle MCP status request.
    async fn handle_mcp_list(&self) {
        let running = self.mcp_manager.running_servers().await;
        let config = self.mcp_manager.config();

        let servers: Vec<McpServerStatus> = config
            .servers
            .keys()
            .map(|name| McpServerStatus {
                name: name.clone(),
                running: running.contains(name),
            })
            .collect();

        self.emit(BridgeOutMessage::McpStatus { servers });
    }

    /// Handle agent switch.
    fn handle_switch_agent(&mut self, agent_name: &str) {
        if self.agents.exists(agent_name) {
            if self.agents.switch(agent_name).is_ok() {
                self.emit(BridgeOutMessage::AgentChanged {
                    agent: agent_name.to_string(),
                });
            }
        } else {
            self.emit(BridgeOutMessage::Error {
                message: format!("Agent not found: {}", agent_name),
            });
        }
    }

    /// Handle model switch.
    fn handle_switch_model(&mut self, model: &str) {
        self.current_model = model.to_string();
        let settings = Settings::new(self.db);
        let _ = settings.set("model", model);
        self.emit(BridgeOutMessage::ModelChanged {
            model: model.to_string(),
        });
    }
}

/// Run in bridge mode (NDJSON over stdio).
pub async fn run_bridge_mode() -> anyhow::Result<()> {
    let db = Database::open()?;
    db.migrate()?;

    let mut bridge = Bridge::new(&db);

    // Start enabled MCP servers
    let _ = bridge.mcp_manager.start_all().await;

    // Emit ready message
    bridge.emit(BridgeOutMessage::Ready {
        version: env!("CARGO_PKG_VERSION").to_string(),
        agent: bridge.agents.current_name(),
        model: bridge.current_model.clone(),
    });

    // Set up stdin reader in a separate thread
    let (tx, mut rx) = mpsc::channel::<BridgeInCommand>(32);

    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            match line {
                Ok(line) if line.is_empty() => continue,
                Ok(line) => match serde_json::from_str::<BridgeInCommand>(&line) {
                    Ok(cmd) => {
                        if tx.blocking_send(cmd).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("{{\"type\":\"error\",\"message\":\"Invalid JSON: {}\"}}", e);
                    }
                },
                Err(_) => break,
            }
        }
    });

    // Main event loop
    while let Some(cmd) = rx.recv().await {
        match cmd {
            BridgeInCommand::Prompt { text, agent, model } => {
                if let Some(a) = agent {
                    bridge.handle_switch_agent(&a);
                }
                if let Some(m) = model {
                    bridge.handle_switch_model(&m);
                }
                bridge.handle_prompt(&text).await;
            }
            BridgeInCommand::Cancel => {
                // TODO: Implement cancellation
                bridge.emit(BridgeOutMessage::Error {
                    message: "Cancellation not yet implemented".to_string(),
                });
            }
            BridgeInCommand::SwitchAgent { agent } => {
                bridge.handle_switch_agent(&agent);
            }
            BridgeInCommand::SwitchModel { model } => {
                bridge.handle_switch_model(&model);
            }
            BridgeInCommand::McpStart { name } => {
                if let Some(n) = name {
                    let _ = bridge.mcp_manager.start_server(&n).await;
                } else {
                    let _ = bridge.mcp_manager.start_all().await;
                }
                bridge.handle_mcp_list().await;
            }
            BridgeInCommand::McpStop { name } => {
                if let Some(n) = name {
                    let _ = bridge.mcp_manager.stop_server(&n).await;
                } else {
                    let _ = bridge.mcp_manager.stop_all().await;
                }
                bridge.handle_mcp_list().await;
            }
            BridgeInCommand::McpList => {
                bridge.handle_mcp_list().await;
            }
            BridgeInCommand::ToolResponse { .. } => {
                // TODO: Implement tool confirmation
            }
            BridgeInCommand::Shutdown => {
                let _ = bridge.mcp_manager.stop_all().await;
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_out_message() {
        let msg = BridgeOutMessage::TextDelta {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("text_delta"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_deserialize_in_command() {
        let json = r#"{"type": "prompt", "text": "Hello"}"#;
        let cmd: BridgeInCommand = serde_json::from_str(json).unwrap();
        match cmd {
            BridgeInCommand::Prompt { text, .. } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Prompt"),
        }
    }

    #[test]
    fn test_deserialize_cancel() {
        let json = r#"{"type": "cancel"}"#;
        let cmd: BridgeInCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, BridgeInCommand::Cancel));
    }
}
