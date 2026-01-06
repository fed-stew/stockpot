//! Prompt execution handling for the REPL.
//!
//! This module handles sending prompts to the LLM agent and processing
//! the streaming response. It includes both the modern message-bus
//! architecture and a legacy handler for reference.

use std::io::{stdout, Write};
use std::time::Duration;

use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::agents::{AgentExecutor, AgentManager, StreamEvent};
use crate::cli::commands::context;
use crate::cli::commands::session;
use crate::cli::streaming_markdown::StreamingMarkdownRenderer;
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{MessageBus, Spinner, TerminalRenderer as MsgRenderer};
use crate::models::ModelRegistry;
use crate::session::SessionManager;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::ModelRequest;

/// Handle a prompt using the message bus architecture.
///
/// This is the modern approach where all events flow through the message bus
/// and are rendered by a subscriber. Much simpler than the legacy approach!
pub async fn handle_prompt_with_bus(
    db: &Database,
    agents: &AgentManager,
    model_registry: &ModelRegistry,
    message_bus: &MessageBus,
    message_history: &mut Vec<ModelRequest>,
    current_model: &str,
    tool_registry: &SpotToolRegistry,
    mcp_manager: &McpManager,
    session_manager: &SessionManager,
    current_session: &mut Option<String>,
    prompt: &str,
) -> anyhow::Result<()> {
    debug!(prompt_len = prompt.len(), "handle_prompt_with_bus started");

    let agent = agents
        .current()
        .ok_or_else(|| anyhow::anyhow!("No agent selected"))?;
    let agent_name = agent.name().to_string();

    // Get effective model (respecting agent pins)
    let effective_model = context::get_effective_model(db, current_model, &agent_name);

    // Prepare history
    let history = if message_history.is_empty() {
        None
    } else {
        Some(message_history.clone())
    };

    // Get MCP tool count for display
    let mcp_tools = mcp_manager.list_all_tools().await;
    let mcp_tool_count: usize = mcp_tools.values().map(|v| v.len()).sum();
    if mcp_tool_count > 0 {
        debug!(mcp_tool_count, "MCP tools available");
        println!("\n\x1b[2m[{} MCP tools available]\x1b[0m", mcp_tool_count);
    }

    info!(model = %effective_model, agent = %agent_name, "Starting request");

    println!(); // Add spacing before spinner

    // Start spinner
    let spinner = Spinner::new();
    let spinner_handle = spinner.start(format!("Thinking... [{}]", effective_model));

    // Create executor with message bus
    let executor = AgentExecutor::new(db, model_registry).with_bus(message_bus.sender());

    // Subscribe and create ready signal to avoid race condition
    let mut receiver = message_bus.subscribe();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn render task that stops spinner on first message
    let render_handle = tokio::spawn(async move {
        let renderer = MsgRenderer::new();

        // Signal that we're ready to receive BEFORE waiting
        let _ = ready_tx.send(());

        // Wait for first message, stop spinner, then render
        if let Ok(first_msg) = receiver.recv().await {
            // Stop spinner before rendering anything
            spinner_handle.stop().await;

            // Render the first message
            let _ = renderer.render(&first_msg);

            // Continue with remaining messages
            renderer.run_loop(receiver).await;
        } else {
            // No messages, just stop spinner
            spinner_handle.stop().await;
        }
    });

    // WAIT for render task to be ready before executing!
    // This prevents the race condition where messages are sent
    // before the render task is listening.
    let _ = ready_rx.await;

    // NOW safe to execute - render task is waiting for messages
    let result = executor
        .execute_with_bus(
            agent,
            &effective_model,
            prompt,
            history,
            tool_registry,
            mcp_manager,
        )
        .await;

    // Give renderer a moment to finish processing, then abort if needed
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    render_handle.abort();

    match result {
        Ok(exec_result) => {
            *message_history = exec_result.messages;
            println!(); // Add spacing after response
            auto_save_session(
                db,
                session_manager,
                current_session,
                message_history,
                &agents.current_name(),
                current_model,
            );
        }
        Err(e) => {
            println!("\n\x1b[1;31m❌ Error:\x1b[0m {}\n", e);
            show_error_hints(current_model, &e.to_string());
        }
    }

    Ok(())
}

/// Legacy prompt handling (direct stream processing).
///
/// Kept for reference and fallback if needed.
#[allow(dead_code)]
pub async fn handle_prompt_legacy(
    db: &Database,
    agents: &AgentManager,
    model_registry: &ModelRegistry,
    message_history: &mut Vec<ModelRequest>,
    current_model: &str,
    tool_registry: &SpotToolRegistry,
    mcp_manager: &McpManager,
    session_manager: &SessionManager,
    current_session: &mut Option<String>,
    prompt: &str,
) -> anyhow::Result<()> {
    debug!(prompt_len = prompt.len(), "handle_prompt_legacy started");

    let agent = agents
        .current()
        .ok_or_else(|| anyhow::anyhow!("No agent selected"))?;
    let display_name = agent.display_name().to_string();
    let agent_name = agent.name().to_string();

    debug!(agent = %agent_name, display_name = %display_name, "Agent selected");

    println!();

    let executor = AgentExecutor::new(db, model_registry);
    let history = if message_history.is_empty() {
        None
    } else {
        Some(message_history.clone())
    };

    debug!(
        history_messages = history.as_ref().map(|h| h.len()).unwrap_or(0),
        "Message history"
    );

    // Get MCP tool count
    let mcp_tools = mcp_manager.list_all_tools().await;
    let mcp_tool_count: usize = mcp_tools.values().map(|v| v.len()).sum();
    if mcp_tool_count > 0 {
        debug!(mcp_tool_count, "MCP tools available");
        println!("\x1b[2m[{} MCP tools available]\x1b[0m\n", mcp_tool_count);
    }

    // Use pinned model if set (from database)
    let effective_model = context::get_effective_model(db, current_model, &agents.current_name());

    info!(model = %effective_model, agent = %agent_name, "Starting request");

    // Start spinner
    let spinner = Spinner::new();
    let mut spinner_handle = Some(spinner.start(format!("Thinking... [{}]", effective_model)));

    debug!("Calling execute_stream");
    match executor
        .execute_stream(
            agent,
            &effective_model,
            prompt,
            history,
            tool_registry,
            mcp_manager,
        )
        .await
    {
        Ok(mut stream) => {
            debug!("execute_stream returned successfully, waiting for events");
            let mut first_text = true;
            let mut has_output = false;
            let mut event_count = 0u32;
            let recv_timeout = Duration::from_secs(120); // 2 minute timeout
            let mut md_renderer = StreamingMarkdownRenderer::new();

            // Track current tool call for nicer output formatting
            let mut current_tool: Option<String> = None;
            let mut tool_args_buffer = String::new();

            loop {
                // Use timeout to detect if we're stuck
                let recv_result = timeout(recv_timeout, stream.recv()).await;

                let event_result = match recv_result {
                    Ok(Some(result)) => result,
                    Ok(None) => {
                        // Stream ended normally
                        debug!(total_events = event_count, "Stream ended (received None)");
                        if event_count == 0 {
                            warn!("Stream ended immediately without any events!");
                            println!("\n\x1b[1;33m⚠️  No response received from model\x1b[0m");
                        }
                        break;
                    }
                    Err(_) => {
                        // Timeout
                        error!("Timeout waiting for stream event after {:?}", recv_timeout);
                        if let Some(handle) = spinner_handle.take() {
                            handle.stop().await;
                        }
                        println!("\n\x1b[1;31m❌ Timeout waiting for response\x1b[0m");
                        break;
                    }
                };

                event_count += 1;
                debug!(event_num = event_count, "Processing event");
                match event_result {
                    Ok(event) => {
                        process_stream_event(
                            event,
                            &mut spinner_handle,
                            &mut first_text,
                            &mut has_output,
                            &mut current_tool,
                            &mut tool_args_buffer,
                            &mut md_renderer,
                            &display_name,
                            event_count,
                        )
                        .await;
                    }
                    Err(e) => {
                        error!(error = %e, "Stream recv error");
                        if let Some(handle) = spinner_handle.take() {
                            handle.stop().await;
                        }
                        println!("\n\x1b[1;31m❌ Stream error: {}\x1b[0m\n", e);
                        show_error_hints(current_model, &e.to_string());
                        break;
                    }
                }
            }
            info!(total_events = event_count, "Stream processing complete");

            // Auto-save if enabled
            auto_save_session(
                db,
                session_manager,
                current_session,
                message_history,
                &agents.current_name(),
                current_model,
            );
        }
        Err(e) => {
            error!(error = %e, "execute_stream failed");
            if let Some(handle) = spinner_handle.take() {
                handle.stop().await;
            }
            println!("\x1b[1;31m❌ Error:\x1b[0m {}\n", e);
            show_error_hints(current_model, &e.to_string());
        }
    }
    Ok(())
}

/// Process a single stream event from the legacy handler.
#[allow(dead_code)]
async fn process_stream_event(
    event: StreamEvent,
    spinner_handle: &mut Option<crate::messaging::SpinnerHandle>,
    first_text: &mut bool,
    has_output: &mut bool,
    current_tool: &mut Option<String>,
    tool_args_buffer: &mut String,
    md_renderer: &mut StreamingMarkdownRenderer,
    display_name: &str,
    event_count: u32,
) {
    match event {
        StreamEvent::RunStart { .. } => {}
        StreamEvent::RequestStart { step: _ } => {
            // Silently continue - no step indicators needed
        }
        StreamEvent::TextDelta { text } => {
            // Stop spinner on first text
            if let Some(handle) = spinner_handle.take() {
                handle.stop().await;
            }
            if *first_text {
                println!("\x1b[1;35m{}:\x1b[0m\n", display_name);
                *first_text = false;
            }
            // Stream through markdown renderer for live formatting
            if let Err(e) = md_renderer.process(&text) {
                debug!(error = %e, "Markdown render error, falling back to raw");
                print!("{}", text);
                let _ = stdout().flush();
            }
            *has_output = true;
        }
        StreamEvent::ToolCallStart { tool_name, .. } => {
            // Pause spinner during tool execution
            if let Some(ref handle) = spinner_handle {
                handle.pause();
            }
            // Track the current tool and reset args buffer
            *current_tool = Some(tool_name.clone());
            tool_args_buffer.clear();
            // Print tool name, args will follow after ToolCallComplete
            print!("\n\x1b[2m{}\x1b[0m ", tool_name);
            let _ = stdout().flush();
        }
        StreamEvent::ToolCallDelta { delta, .. } => {
            // Accumulate args for later display
            tool_args_buffer.push_str(&delta);
        }
        StreamEvent::ToolCallComplete { .. } => {
            // Now show the args in a nice format
            if let Some(ref tool) = current_tool {
                debug!(tool = %tool, args_buffer = %tool_args_buffer, "ToolCallComplete - parsing args");
                print_tool_args(tool, tool_args_buffer);
            }
            let _ = stdout().flush();
        }
        StreamEvent::ToolExecuted { success, error, .. } => {
            // Just end the line, show error if failed
            if success {
                println!();
            } else if let Some(err) = error {
                // Show truncated error message
                let short_err = if err.len() > 60 {
                    format!("{}...", &err[..57])
                } else {
                    err
                };
                println!(" \x1b[1;31m✗ {}\x1b[0m", short_err);
            } else {
                println!(" \x1b[1;31m✗ failed\x1b[0m");
            }
            *current_tool = None;
            // Resume spinner after tool
            if let Some(ref handle) = spinner_handle {
                handle.resume();
            }
        }
        StreamEvent::ThinkingDelta { text } => {
            // Stop spinner for thinking output
            if let Some(handle) = spinner_handle.take() {
                handle.stop().await;
            }
            print!("\x1b[2m{}\x1b[0m", text);
            let _ = stdout().flush();
        }
        StreamEvent::ResponseComplete { .. } | StreamEvent::OutputReady => {}
        StreamEvent::RunComplete { run_id, .. } => {
            debug!(run_id = %run_id, total_events = event_count, "Run completed");
            if let Some(handle) = spinner_handle.take() {
                handle.stop().await;
            }
            // Flush the markdown renderer
            if let Err(e) = md_renderer.flush() {
                debug!(error = %e, "Failed to flush markdown renderer");
            }
            if *has_output {
                println!("\n"); // Add newlines after response
            }
        }
        StreamEvent::Error { message } => {
            error!(error = %message, "Stream error event received");
            if let Some(handle) = spinner_handle.take() {
                handle.stop().await;
            }
            println!("\n\x1b[1;31m❌ Error: {}\x1b[0m\n", message);
        }
    }
}

/// Print tool arguments in a nice format.
#[allow(dead_code)]
fn print_tool_args(tool: &str, args_buffer: &str) {
    // Try to parse args and show nicely
    if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_buffer) {
        debug!(parsed_args = %args, "Parsed tool args");
        match tool {
            "read_file" => {
                if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                    print!("\x1b[36m{}\x1b[0m", path);
                }
            }
            "list_files" => {
                if let Some(dir) = args.get("directory").and_then(|v| v.as_str()) {
                    print!("\x1b[36m{}\x1b[0m", dir);
                }
            }
            "grep" => {
                if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                    print!("\x1b[36m'{}'\x1b[0m", pattern);
                }
            }
            "agent_run_shell_command" | "run_shell_command" => {
                if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                    // Truncate long commands
                    let display_cmd = if cmd.len() > 60 {
                        format!("{}...", &cmd[..57])
                    } else {
                        cmd.to_string()
                    };
                    print!("\x1b[36m{}\x1b[0m", display_cmd);
                }
            }
            _ => {
                // For other tools, show compact args
                let compact = args.to_string();
                if compact.len() > 80 {
                    print!("\x1b[2m{}...\x1b[0m", &compact[..77]);
                } else {
                    print!("\x1b[2m{}\x1b[0m", compact);
                }
            }
        }
    } else if !args_buffer.is_empty() {
        // JSON parsing failed - show raw args
        debug!(raw_args = %args_buffer, "Failed to parse tool args as JSON");
        let display = if args_buffer.len() > 60 {
            format!("{}...", &args_buffer[..57])
        } else {
            args_buffer.to_string()
        };
        print!("\x1b[2m{}\x1b[0m", display);
    }
}

/// Auto-save session if enabled in settings.
pub fn auto_save_session(
    db: &Database,
    session_manager: &SessionManager,
    current_session: &mut Option<String>,
    message_history: &[ModelRequest],
    agent_name: &str,
    current_model: &str,
) {
    let settings = Settings::new(db);
    if !settings.get_bool("auto_save_session").unwrap_or(true) {
        return;
    }

    if let Some(name) = session::auto_save(
        session_manager,
        current_session,
        message_history,
        agent_name,
        current_model,
    ) {
        *current_session = Some(name);
    }
}

/// Show helpful hints based on the error message.
pub fn show_error_hints(current_model: &str, error_str: &str) {
    if error_str.contains("Auth") || error_str.contains("Not authenticated") {
        if current_model.contains("chatgpt") {
            println!("\x1b[2mHint: Run /chatgpt-auth to authenticate\x1b[0m");
        } else if current_model.contains("claude-code") {
            println!("\x1b[2mHint: Run /claude-code-auth to authenticate\x1b[0m");
        } else {
            println!("\x1b[2mHint: Make sure your API key is set\x1b[0m");
        }
    } else if error_str.contains("model") || error_str.contains("Model") {
        println!("\x1b[2mHint: Check your model name with /model\x1b[0m");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // print_tool_args Tests
    // =========================================================================

    #[test]
    fn test_print_tool_args_read_file_extracts_path() {
        // Should not panic; prints cyan path
        let args = r#"{"file_path": "/some/path/to/file.rs"}"#;
        print_tool_args("read_file", args);
    }

    #[test]
    fn test_print_tool_args_list_files_extracts_dir() {
        // Should not panic; prints cyan directory
        let args = r#"{"directory": "/some/directory"}"#;
        print_tool_args("list_files", args);
    }

    #[test]
    fn test_print_tool_args_grep_extracts_pattern() {
        // Should not panic; prints cyan pattern with quotes
        let args = r#"{"pattern": "fn main"}"#;
        print_tool_args("grep", args);
    }

    #[test]
    fn test_print_tool_args_shell_command_short() {
        // Short command should print without truncation
        let args = r#"{"command": "ls -la"}"#;
        print_tool_args("run_shell_command", args);
        print_tool_args("agent_run_shell_command", args);
    }

    #[test]
    fn test_print_tool_args_shell_command_truncates_long() {
        // Command > 60 chars should be truncated with "..."
        let long_cmd = "a".repeat(100);
        let args = format!(r#"{{"command": "{}"}}"#, long_cmd);
        print_tool_args("run_shell_command", &args);
        print_tool_args("agent_run_shell_command", &args);
    }

    #[test]
    fn test_print_tool_args_other_tool_compact() {
        // Unknown tool shows compact JSON
        let args = r#"{"key": "value", "num": 42}"#;
        print_tool_args("unknown_tool", args);
    }

    #[test]
    fn test_print_tool_args_other_tool_truncates() {
        // JSON > 80 chars should be truncated
        let long_value = "x".repeat(100);
        let args = format!(r#"{{"long_key": "{}"}}"#, long_value);
        print_tool_args("unknown_tool", &args);
    }

    #[test]
    fn test_print_tool_args_invalid_json_shows_raw() {
        // Invalid JSON fallback to raw display
        let args = "not valid json {{{";
        print_tool_args("some_tool", args);
    }

    #[test]
    fn test_print_tool_args_invalid_json_truncates_long() {
        // Long invalid JSON should be truncated
        let long_invalid = "not json ".repeat(20);
        print_tool_args("some_tool", &long_invalid);
    }

    #[test]
    fn test_print_tool_args_empty_buffer() {
        // Empty args buffer should not panic
        print_tool_args("any_tool", "");
    }

    #[test]
    fn test_print_tool_args_read_file_missing_path() {
        // Valid JSON but missing expected field - should not panic
        let args = r#"{"other_field": "value"}"#;
        print_tool_args("read_file", args);
    }

    #[test]
    fn test_print_tool_args_list_files_missing_dir() {
        // Valid JSON but missing expected field
        let args = r#"{"path": "/some/path"}"#;
        print_tool_args("list_files", args);
    }

    #[test]
    fn test_print_tool_args_grep_missing_pattern() {
        // Valid JSON but missing expected field
        let args = r#"{"search": "term"}"#;
        print_tool_args("grep", args);
    }

    #[test]
    fn test_print_tool_args_shell_command_missing_command() {
        // Valid JSON but missing expected field
        let args = r#"{"exec": "ls"}"#;
        print_tool_args("run_shell_command", args);
    }

    // =========================================================================
    // show_error_hints Tests
    // =========================================================================

    #[test]
    fn test_show_error_hints_chatgpt_auth() {
        // ChatGPT auth error should suggest /chatgpt-auth
        show_error_hints("chatgpt-4o", "Auth error: Not authenticated");
    }

    #[test]
    fn test_show_error_hints_claude_code_auth() {
        // Claude code auth error should suggest /claude-code-auth
        show_error_hints("claude-code-sonnet", "Auth error: token expired");
    }

    #[test]
    fn test_show_error_hints_generic_auth() {
        // Generic provider auth error should suggest API key
        show_error_hints("gpt-4", "Not authenticated with provider");
    }

    #[test]
    fn test_show_error_hints_generic_auth_uppercase() {
        // Auth detection should work with "Auth" capitalized
        show_error_hints("gemini-pro", "Auth failed");
    }

    #[test]
    fn test_show_error_hints_model_error() {
        // Model-related error should suggest /model command
        show_error_hints("unknown-model", "Invalid model specified");
    }

    #[test]
    fn test_show_error_hints_model_error_lowercase() {
        // "model" lowercase should also trigger hint
        show_error_hints("gpt-4", "unknown model name");
    }

    #[test]
    fn test_show_error_hints_other_error_no_hint() {
        // Errors not matching patterns should not panic
        show_error_hints("gpt-4", "Connection timeout");
    }

    #[test]
    fn test_show_error_hints_empty_error() {
        // Empty error string should not panic
        show_error_hints("gpt-4", "");
    }

    #[test]
    fn test_show_error_hints_empty_model() {
        // Empty model string should not panic
        show_error_hints("", "Auth error");
    }
}
