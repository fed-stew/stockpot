//! Agent execution for ChatApp
//!
//! This module handles sending messages and executing agents:
//! - `send_message()` - Prepare and send a user message
//! - `execute_agent()` - Run an agent with the current context

use std::rc::Rc;
use std::sync::Arc;

use gpui::{AsyncApp, Context, WeakEntity, Window};

use crate::agents::{AgentExecutor, AgentManager};
use crate::config::{PdfMode, Settings};
use crate::db::Database;
use crate::mcp::McpManager;
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::messages::ImageMediaType;

use super::{ChatApp, PendingAttachment, MAX_IMAGE_DIMENSION};

impl ChatApp {
    /// Handle sending a message with real agent execution
    pub(super) fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.input_state.read(cx).value().to_string();
        let text = content.trim().to_string();
        let has_attachments = !self.pending_attachments.is_empty();

        // Need either text or attachments
        if text.is_empty() && !has_attachments {
            return;
        }

        if self.is_generating {
            return;
        }

        // Build the message including attachments
        let mut full_message = text.clone();

        // Add file references for non-image attachments
        let file_refs: Vec<String> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::File(f) => Some(format!("File: {}", f.path.display())),
                _ => None,
            })
            .collect();

        if !file_refs.is_empty() {
            if !full_message.is_empty() {
                full_message.push_str("\n\n");
            }
            full_message.push_str(&file_refs.join("\n"));
        }

        // Collect images for vision model support
        // Each image is (PNG bytes, ImageMediaType::Png)
        let mut images: Vec<(Vec<u8>, ImageMediaType)> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::Image(img) => {
                    Some((img.processed_data.clone(), ImageMediaType::Png))
                }
                _ => None,
            })
            .collect();

        // Log collected images
        tracing::info!(
            image_count = images.len(),
            total_bytes = images.iter().map(|(b, _)| b.len()).sum::<usize>(),
            "Collected images for sending"
        );

        // Process PDF attachments based on current mode
        use crate::gui::pdf_processing::{extract_pdf_text, render_pdf_to_images};

        let pdf_attachments: Vec<_> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::Pdf(pdf) => Some(pdf.clone()),
                _ => None,
            })
            .collect();

        if !pdf_attachments.is_empty() {
            match self.pdf_mode {
                PdfMode::Image => {
                    // IMAGE MODE: Render PDF pages to images
                    for pdf in &pdf_attachments {
                        tracing::info!(
                            filename = %pdf.filename,
                            page_count = pdf.page_count,
                            "Converting PDF to images"
                        );
                        match render_pdf_to_images(&pdf.path, MAX_IMAGE_DIMENSION) {
                            Ok(page_images) => {
                                let pages_rendered = page_images.len();
                                for page in page_images {
                                    images.push((page.processed_data, ImageMediaType::Png));
                                }
                                tracing::info!(
                                    filename = %pdf.filename,
                                    pages_rendered = pages_rendered,
                                    "PDF pages converted to images"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    filename = %pdf.filename,
                                    error = %e,
                                    "Failed to render PDF to images"
                                );
                                self.error_message = Some(format!(
                                    "Failed to convert PDF '{}': {}",
                                    pdf.filename, e
                                ));
                            }
                        }
                    }
                }
                PdfMode::TextExtract => {
                    // TEXT MODE: Extract text and append to message
                    for pdf in &pdf_attachments {
                        tracing::info!(
                            filename = %pdf.filename,
                            page_count = pdf.page_count,
                            "Extracting text from PDF"
                        );
                        match extract_pdf_text(&pdf.path) {
                            Ok(text) => {
                                if !full_message.is_empty() {
                                    full_message.push_str("\n\n");
                                }
                                full_message.push_str(&format!(
                                    "--- PDF: {} ({} pages) ---\n{}",
                                    pdf.filename, pdf.page_count, text
                                ));
                                tracing::info!(
                                    filename = %pdf.filename,
                                    text_length = text.len(),
                                    "PDF text extracted"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    filename = %pdf.filename,
                                    error = %e,
                                    "Failed to extract PDF text"
                                );
                                self.error_message = Some(format!(
                                    "Failed to extract text from '{}': {}",
                                    pdf.filename, e
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Check if the model supports vision - if not, warn and strip images
        let has_images = !images.is_empty();
        if has_images {
            // Get the effective model for the current agent
            let effective_model_name = {
                let settings = Settings::new(&self.db);
                settings
                    .get_agent_pinned_model(&self.current_agent)
                    .unwrap_or_else(|| self.current_model.clone())
            };

            // Check if model supports vision
            let model_config = self.model_registry.get(&effective_model_name);

            // Log the vision check for debugging
            tracing::info!(
                model_name = %effective_model_name,
                model_found = model_config.is_some(),
                supports_vision = model_config.map(|m| m.supports_vision),
                "Vision support check"
            );

            // Default to TRUE if model not found (assume modern models support vision)
            let supports_vision = model_config.map(|m| m.supports_vision).unwrap_or(true);

            if !supports_vision {
                // Show warning to user
                tracing::warn!(
                    model_name = %effective_model_name,
                    "Model doesn't support vision, stripping images"
                );
                self.error_message = Some(format!(
                    "⚠️ Model '{}' doesn't support images. Images removed from message.",
                    effective_model_name
                ));
                // Strip images
                images.clear();
            }
        }

        // Add user message to conversation
        if has_attachments {
            let attachment_note = format!(
                "{}\n\n📎 {} attachment(s)",
                if text.is_empty() {
                    "(Image attached)".to_string()
                } else {
                    text.clone()
                },
                self.pending_attachments.len()
            );
            self.conversation.add_user_message(&attachment_note);
        } else {
            self.conversation.add_user_message(&text);
        }

        // Clear input and attachments
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.pending_attachments.clear();

        // Execute agent with message and images
        self.execute_agent(full_message, images, cx);

        cx.notify();
    }

    /// Execute the agent with the given prompt and optional images
    pub(super) fn execute_agent(
        &mut self,
        prompt: String,
        images: Vec<(Vec<u8>, ImageMediaType)>,
        cx: &mut Context<Self>,
    ) {
        // Bundle all data that needs to be moved into the async closure
        // This ensures everything is captured as a single unit
        struct ExecuteData {
            agent_name: String,
            db: Rc<Database>,
            agents: Arc<AgentManager>,
            model_registry: Arc<ModelRegistry>,
            default_model: String,
            tool_registry: Arc<SpotToolRegistry>,
            mcp_manager: Arc<McpManager>,
            message_bus_sender: crate::messaging::MessageSender,
            prompt: String,
            images: Vec<(Vec<u8>, ImageMediaType)>,
            history: Option<Vec<serdes_ai_core::ModelRequest>>,
        }

        let data = ExecuteData {
            agent_name: self.current_agent.clone(),
            db: self.db.clone(),
            agents: self.agents.clone(),
            model_registry: self.model_registry.clone(),
            default_model: self.current_model.clone(),
            tool_registry: self.tool_registry.clone(),
            mcp_manager: self.mcp_manager.clone(),
            message_bus_sender: self.message_bus.sender(),
            prompt,
            images,
            history: if self.message_history.is_empty() {
                None
            } else {
                Some(self.message_history.clone())
            },
        };

        // Log BEFORE the spawn to verify data is correct in struct
        tracing::info!(
            image_count_in_struct = data.images.len(),
            prompt_len_in_struct = data.prompt.len(),
            "execute_agent: data struct created BEFORE spawn"
        );

        self.is_generating = true;
        self.error_message = None;

        // Update context before spawning - gives initial estimate
        self.update_context_usage();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            // Destructure the data bundle
            let ExecuteData {
                agent_name,
                db,
                agents,
                model_registry,
                default_model,
                tool_registry,
                mcp_manager,
                message_bus_sender,
                prompt,
                images,
                history,
            } = data;

            // Log images inside async block to verify they survived the move
            tracing::info!(
                image_count = images.len(),
                prompt_len = prompt.len(),
                "execute_agent async: checking images before execution"
            );

            // Look up the agent by name inside the async block
            let Some(agent) = agents.get(&agent_name) else {
                this.update(cx, |app, cx| {
                    app.is_generating = false;
                    app.error_message = Some("No agent selected".to_string());
                    cx.notify();
                })
                .ok();
                return;
            };

            // Create executor with message bus
            let executor = AgentExecutor::new(&db, &model_registry).with_bus(message_bus_sender);

            // Get the effective model for this agent (pinned or default)
            let effective_model = {
                let settings = Settings::new(&db);
                settings
                    .get_agent_pinned_model(&agent_name)
                    .unwrap_or_else(|| default_model.clone())
            };

            // Execute the agent - use execute_with_images if we have images
            tracing::info!(
                images_empty = images.is_empty(),
                "execute_agent: about to choose execution path"
            );
            let result = if images.is_empty() {
                executor
                    .execute_with_bus(
                        agent,
                        &effective_model,
                        &prompt,
                        history,
                        &tool_registry,
                        &mcp_manager,
                    )
                    .await
            } else {
                executor
                    .execute_with_images(
                        agent,
                        &effective_model,
                        &prompt,
                        &images,
                        history,
                        &tool_registry,
                        &mcp_manager,
                    )
                    .await
            };

            // Update state based on result
            tracing::info!("Execution async task finished, calling this.update()");
            this.update(cx, |app, cx| {
                tracing::info!("Inside this.update() callback");
                app.is_generating = false;
                match result {
                    Ok(exec_result) => {
                        tracing::info!(
                            messages_count = exec_result.messages.len(),
                            "Execution completed, updating message history"
                        );
                        if !exec_result.messages.is_empty() {
                            app.message_history = exec_result.messages;
                            app.update_context_usage();
                        }
                    }
                    Err(e) => {
                        app.error_message = Some(e.to_string());
                        app.conversation
                            .append_to_current(&format!("\n\n❌ Error: {}", e));
                        app.conversation.finish_current_message();
                    }
                }
                cx.notify();
            })
            .map_err(|e| tracing::error!("this.update() failed: {:?}", e))
            .ok();
        })
        .detach();
    }
}
