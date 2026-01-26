//! System Executions sidebar for terminal management.
//!
//! This module provides a collapsible right-side panel showing all
//! active terminal processes (both LLM-spawned and user shells).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, rgba, Context, Entity, MouseButton, ScrollHandle, SharedString,
    Styled, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::ChatApp;
use crate::gui::components::{scrollbar, ScrollbarDragState, TerminalView};
use stockpot_core::config::Settings;
use stockpot_core::terminal::{
    headless_env, interactive_env, spawn_pty, validate_command, CommandValidation, ProcessKind,
    ProcessSnapshot, PtyConfig, PtyEvent, RiskLevel, SystemExecRequest, SystemExecResponse,
    SystemExecStore, TerminalEventBridge, TerminalSize,
};

/// Default width of the system executions sidebar
const DEFAULT_SIDEBAR_WIDTH: f32 = 320.0;

/// Minimum sidebar width
const MIN_SIDEBAR_WIDTH: f32 = 200.0;

/// Maximum sidebar width
const MAX_SIDEBAR_WIDTH: f32 = 600.0;

/// Width of the resize handle
const RESIZE_HANDLE_WIDTH: f32 = 6.0;

/// Max lines of output to show in preview
const OUTPUT_PREVIEW_LINES: usize = 8;

/// Pending command awaiting user approval
#[derive(Debug)]
pub struct PendingApproval {
    pub request_id: u64,
    pub command: String,
    pub cwd: Option<String>,
    pub validation: CommandValidation,
}

/// State for the System Executions sidebar
pub struct SystemExecutionsState {
    /// Whether the sidebar is visible
    pub visible: bool,
    /// Process store for tracking terminals
    pub store: Arc<SystemExecStore>,
    /// Active terminals keyed by process_id
    pub terminals: HashMap<String, TerminalHandle>,
    /// Scroll handle for the terminal list
    pub scroll_handle: ScrollHandle,
    /// Scrollbar drag state
    pub scrollbar_drag: std::rc::Rc<ScrollbarDragState>,
    /// Which terminal is currently expanded (shows full output)
    pub expanded_terminal: Option<String>,
    /// Counter for generating unique process IDs
    next_process_id: u64,
    /// Channel sender for tool requests (clone this for ToolContext)
    pub request_tx: mpsc::UnboundedSender<SystemExecRequest>,
    /// Channel receiver for tool requests (process in event loop)
    pub request_rx: Option<mpsc::UnboundedReceiver<SystemExecRequest>>,
    /// Pending command awaiting user approval
    pub pending_approval: Option<PendingApproval>,
    /// Current sidebar width
    pub sidebar_width: f32,
    /// Whether we're currently resizing
    pub is_resizing: bool,
    /// Starting X position for resize drag
    pub resize_start_x: Option<f32>,
    /// Starting width for resize drag
    pub resize_start_width: Option<f32>,
    /// Input state entities per terminal (keyed by process_id) - for text-based fallback
    pub terminal_inputs: HashMap<String, Entity<InputState>>,
    /// Terminal view entities (keyed by process_id) - for graphical terminal rendering
    pub terminal_views: HashMap<String, Entity<TerminalView>>,
    /// Custom heights per terminal (process_id -> height in pixels)
    pub terminal_heights: HashMap<String, f32>,
    /// Which terminal is currently being height-resized
    pub resizing_terminal: Option<String>,
    /// Starting Y position for terminal height resize
    pub resize_start_y: Option<f32>,
    /// Starting height for terminal height resize
    pub resize_start_height: Option<f32>,
    /// Whether to show the terminal name dialog
    pub show_name_dialog: bool,
    /// Input state for terminal name dialog
    pub name_input: Option<Entity<InputState>>,
}

impl Default for SystemExecutionsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to an active terminal
pub struct TerminalHandle {
    /// Process ID
    #[allow(dead_code)]
    pub process_id: String,
    /// Channel to send input to the PTY
    pub writer_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Channel to resize the PTY
    #[allow(dead_code)]
    pub resize_tx: mpsc::UnboundedSender<portable_pty::PtySize>,
    /// Alacritty terminal for rendering (optional - Some for graphical terminals)
    /// Reserved for future use (direct terminal manipulation)
    #[allow(dead_code)]
    pub term: Option<
        Arc<
            parking_lot::FairMutex<
                alacritty_terminal::term::Term<stockpot_core::terminal::TerminalEventBridge>,
            >,
        >,
    >,
}

impl SystemExecutionsState {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        Self {
            visible: false,
            store: Arc::new(SystemExecStore::new()),
            terminals: HashMap::new(),
            scroll_handle: ScrollHandle::new(),
            scrollbar_drag: std::rc::Rc::new(ScrollbarDragState::default()),
            expanded_terminal: None,
            next_process_id: 1,
            request_tx,
            request_rx: Some(request_rx),
            pending_approval: None,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            is_resizing: false,
            resize_start_x: None,
            resize_start_width: None,
            terminal_inputs: HashMap::new(),
            terminal_views: HashMap::new(),
            terminal_heights: HashMap::new(),
            resizing_terminal: None,
            resize_start_y: None,
            resize_start_height: None,
            show_name_dialog: false,
            name_input: None,
        }
    }

    /// Take the request receiver (for use in event loop).
    pub fn take_request_rx(&mut self) -> Option<mpsc::UnboundedReceiver<SystemExecRequest>> {
        self.request_rx.take()
    }

    /// Toggle sidebar visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Generate a unique process ID
    pub fn generate_process_id(&mut self) -> String {
        let id = format!("proc-{}", self.next_process_id);
        self.next_process_id += 1;
        id
    }

    /// Get visible terminal snapshots sorted:
    /// 1. Named terminals first (alphabetically by name)
    /// 2. Then unnamed terminals by start time (oldest first)
    /// Hidden terminals (like finished LLM commands) are filtered out
    pub fn get_sorted_snapshots(&self) -> Vec<ProcessSnapshot> {
        let mut snapshots: Vec<_> = self
            .store
            .all_snapshots()
            .into_iter()
            .filter(|s| s.visible) // Only show visible terminals
            .collect();

        snapshots.sort_by(|a, b| {
            match (&a.name, &b.name) {
                // Both have names - sort alphabetically
                (Some(name_a), Some(name_b)) => name_a.cmp(name_b),
                // Named terminals come before unnamed
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                // Both unnamed - sort by start time (oldest first)
                (None, None) => a.started_at_ms.cmp(&b.started_at_ms),
            }
        });

        snapshots
    }

    /// Update a terminal's snapshot in the store
    #[allow(dead_code)]
    pub fn update_snapshot(&self, snapshot: ProcessSnapshot) {
        self.store.upsert_process(snapshot);
    }

    /// Remove a terminal from tracking
    pub fn remove_terminal(&mut self, process_id: &str) {
        self.terminals.remove(process_id);
        self.store.remove_process(process_id);
        if self.expanded_terminal.as_deref() == Some(process_id) {
            self.expanded_terminal = None;
        }
    }
}

impl ChatApp {
    /// Render the System Executions sidebar panel (called from main content area)
    pub(crate) fn render_system_executions_panel(
        &self,
        cx: &Context<Self>,
        width: f32,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let state = &self.system_executions;

        // Get snapshots for rendering
        let snapshots = state.get_sorted_snapshots();

        div()
            .flex()
            .flex_row()
            .h_full()
            // Resize handle on the left edge
            .child(self.render_resize_handle(cx))
            // Main sidebar content
            .child(
                div()
                    .w(px(width - RESIZE_HANDLE_WIDTH))
                    .h_full()
                    .border_l_1()
                    .border_color(theme.border)
                    .bg(theme.panel_background)
                    .flex()
                    .flex_col()
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(16.))
                            .py(px(12.))
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(div().text_size(px(14.)).child("⚡"))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .child("System Executions"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("close-system-exec")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tool_card))
                                    .text_color(theme.text_muted)
                                    .text_size(px(12.))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.system_executions.visible = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child("✕"),
                            ),
                    )
                    // Terminal count badge
                    .child(
                        div()
                            .px(px(16.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(theme.border)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_size(px(12.)).text_color(theme.text_muted).child(
                                format!(
                                    "{} process{}",
                                    snapshots.len(),
                                    if snapshots.len() == 1 { "" } else { "es" }
                                ),
                            ))
                            .child(
                                div()
                                    .id("new-terminal-btn")
                                    .px(px(10.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .bg(theme.accent)
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(11.))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.9))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            // Create input state if needed
                                            if this.system_executions.name_input.is_none() {
                                                let input = cx.new(|cx| {
                                                    InputState::new(window, cx)
                                                        .placeholder("e.g., dev-server, build-watch...")
                                                });
                                                // Subscribe to Enter key to create terminal
                                                cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
                                                    if let InputEvent::PressEnter { secondary: false } = event {
                                                        let name = input.read(cx).value().to_string();
                                                        let name = if name.trim().is_empty() {
                                                            None
                                                        } else {
                                                            Some(name.trim().to_string())
                                                        };
                                                        this.system_executions.show_name_dialog = false;
                                                        this.system_executions.name_input = None;
                                                        // Get window from windows list and spawn terminal
                                                        if let Some(window_handle) = cx.windows().first().cloned() {
                                                            cx.spawn(async move |this: gpui::WeakEntity<ChatApp>, cx: &mut gpui::AsyncApp| {
                                                                let _ = cx.update_window(window_handle, |_, window, cx| {
                                                                    let _ = this.update(cx, |this, cx| {
                                                                        this.spawn_user_terminal_with_name(name, window, cx);
                                                                    });
                                                                });
                                                            }).detach();
                                                        }
                                                        cx.notify();
                                                    }
                                                }).detach();
                                                this.system_executions.name_input = Some(input);
                                            }
                                            this.system_executions.show_name_dialog = true;
                                            cx.notify();
                                        }),
                                    )
                                    .child("+ New Shell"),
                            ),
                    )
                    // Name terminal dialog
                    .when(state.show_name_dialog, |d| {
                        d.child(
                            div()
                                .w_full()
                                .px(px(16.))
                                .py(px(12.))
                                .bg(theme.tool_card)
                                .border_b_1()
                                .border_color(theme.border)
                                .flex()
                                .flex_col()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text)
                                        .child("Name your terminal (optional):")
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(8.))
                                        .when_some(state.name_input.clone(), |d, input_entity| {
                                            d.child(
                                                div()
                                                    .flex_1()
                                                    .child(Input::new(&input_entity))
                                            )
                                        })
                                        .child(
                                            div()
                                                .id("create-terminal-btn")
                                                .px(px(12.))
                                                .py(px(6.))
                                                .rounded(px(4.))
                                                .bg(theme.accent)
                                                .text_color(rgb(0xffffff))
                                                .text_size(px(11.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                                    let name = this.system_executions.name_input
                                                        .as_ref()
                                                        .map(|input| input.read(cx).value().to_string())
                                                        .filter(|s| !s.trim().is_empty())
                                                        .map(|s| s.trim().to_string());
                                                    this.system_executions.show_name_dialog = false;
                                                    this.system_executions.name_input = None;
                                                    this.spawn_user_terminal_with_name(name, window, cx);
                                                    cx.notify();
                                                }))
                                                .child("Create")
                                        )
                                        .child(
                                            div()
                                                .id("cancel-terminal-btn")
                                                .px(px(12.))
                                                .py(px(6.))
                                                .rounded(px(4.))
                                                .bg(theme.border)
                                                .text_color(theme.text_muted)
                                                .text_size(px(11.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.system_executions.show_name_dialog = false;
                                                    this.system_executions.name_input = None;
                                                    cx.notify();
                                                }))
                                                .child("Cancel")
                                        )
                                )
                        )
                    })
                    // Terminal list
                    .child(
                        div()
                            .id("system-exec-list-wrap")
                            .flex()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("system-exec-list")
                                    .flex_1()
                                    .min_h(px(0.))
                                    .overflow_y_scroll()
                                    .track_scroll(&state.scroll_handle)
                                    .px(px(12.))
                                    .py(px(8.))
                                    .child(
                                        div().flex().flex_col().gap(px(8.)).children(
                                            snapshots
                                                .iter()
                                                .map(|snap| self.render_terminal_card(snap, cx)),
                                        ),
                                    ),
                            )
                            .child(scrollbar(
                                state.scroll_handle.clone(),
                                state.scrollbar_drag.clone(),
                                theme.clone(),
                            )),
                    ),
            )
    }

    /// Render the resize handle for the sidebar
    fn render_resize_handle(&self, cx: &Context<Self>) -> impl IntoElement {
        let _theme = self.theme.clone(); // May be used for hover color
        let is_resizing = self.system_executions.is_resizing;

        div()
            .id("sidebar-resize-handle")
            .w(px(RESIZE_HANDLE_WIDTH))
            .h_full()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .bg(if is_resizing {
                rgba(0x0078d466) // accent with alpha
            } else {
                rgba(0x00000000) // transparent
            })
            .hover(|s| s.bg(rgba(0x0078d433)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.system_executions.is_resizing = true;
                    this.system_executions.resize_start_x = Some(event.position.x.into());
                    this.system_executions.resize_start_width =
                        Some(this.system_executions.sidebar_width);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.system_executions.is_resizing = false;
                    this.system_executions.resize_start_x = None;
                    this.system_executions.resize_start_width = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if this.system_executions.is_resizing {
                    if let (Some(start_x), Some(start_width)) = (
                        this.system_executions.resize_start_x,
                        this.system_executions.resize_start_width,
                    ) {
                        let current_x: f32 = event.position.x.into();
                        let delta = start_x - current_x;
                        let new_width =
                            (start_width + delta).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                        this.system_executions.sidebar_width = new_width;
                        cx.notify();
                    }
                }
            }))
    }

    /// Render a single terminal card
    fn render_terminal_card(
        &self,
        snapshot: &ProcessSnapshot,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let process_id = snapshot.process_id.clone();
        let is_expanded = self.system_executions.expanded_terminal.as_ref() == Some(&process_id);

        // Status indicator
        let (status_color, status_text) = if let Some(code) = snapshot.exit_code {
            if code == 0 {
                (theme.success, format!("Exit: {}", code))
            } else {
                (theme.error, format!("Exit: {}", code))
            }
        } else {
            (theme.accent, "Running".to_string())
        };

        // Output preview (last few lines)
        let output_preview = get_output_preview(&snapshot.output, OUTPUT_PREVIEW_LINES);
        let has_output = !snapshot.output.trim().is_empty();

        // Kind badge
        let kind_label = match snapshot.kind {
            ProcessKind::Llm => "LLM",
            ProcessKind::User => "User",
        };

        let process_id_for_expand = process_id.clone();
        let process_id_for_copy = process_id.clone();
        let process_id_for_kill = process_id.clone();
        let process_id_for_close = process_id.clone();
        let output_for_copy = snapshot.output.clone();
        let is_running = snapshot.exit_code.is_none();

        div()
            .id(SharedString::from(format!("term-card-{}", process_id)))
            .flex()
            .flex_col()
            .p(px(10.))
            .rounded(px(6.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.tool_card)
            // Header row
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            // Status dot
                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(status_color))
                            // Terminal name or process ID
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child(
                                                snapshot
                                                    .name
                                                    .clone()
                                                    .unwrap_or_else(|| process_id.clone()),
                                            ),
                                    )
                                    // Show process_id underneath if terminal has a name
                                    .when_some(snapshot.name.as_ref(), |d, _| {
                                        d.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.text_muted)
                                                .child(format!("({})", process_id)),
                                        )
                                    }),
                            )
                            // Kind badge
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(if snapshot.kind == ProcessKind::Llm {
                                        rgba(0x0078d433) // accent with alpha
                                    } else {
                                        rgba(0x4ec9b033) // success with alpha
                                    })
                                    .text_size(px(10.))
                                    .text_color(if snapshot.kind == ProcessKind::Llm {
                                        theme.accent
                                    } else {
                                        theme.success
                                    })
                                    .child(kind_label),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.text_muted)
                            .child(status_text),
                    ),
            )
            // Terminal view - use graphical terminal if available, else text fallback
            .child({
                // Get custom height or use default
                let default_height = 250.0;
                let terminal_height = self
                    .system_executions
                    .terminal_heights
                    .get(&process_id)
                    .copied()
                    .unwrap_or(default_height);

                if let Some(terminal_view) = self.system_executions.terminal_views.get(&process_id)
                {
                    // Graphical terminal rendering - responsive width, custom height
                    div()
                        .id(SharedString::from(format!("term-container-{}", process_id)))
                        .w_full()
                        .pr(px(14.)) // Right padding for vertical scrollbar visibility
                        .pb(px(14.)) // Bottom padding for horizontal scrollbar visibility
                        .h(px(terminal_height))
                        .overflow_y_scroll()
                        .child(terminal_view.clone())
                        .into_any_element()
                } else if has_output {
                    // Text fallback for terminals without graphical view
                    div()
                        .id(SharedString::from(format!(
                            "term-output-{}",
                            process_id_for_expand.clone()
                        )))
                        .w_full()
                        .max_h(if is_expanded { px(300.) } else { px(100.) })
                        .overflow_y_scroll()
                        .overflow_x_scroll()
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(rgb(0x1a1a1a))
                        .border_1()
                        .border_color(rgba(0x00000033))
                        .font_family("monospace")
                        .text_size(px(12.))
                        .text_color(rgb(0xcccccc))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let current = this.system_executions.expanded_terminal.clone();
                                if current.as_ref() == Some(&process_id_for_expand) {
                                    this.system_executions.expanded_terminal = None;
                                } else {
                                    this.system_executions.expanded_terminal =
                                        Some(process_id_for_expand.clone());
                                }
                                cx.notify();
                            }),
                        )
                        .child(div().whitespace_nowrap().child(if is_expanded {
                            strip_ansi(&snapshot.output)
                        } else {
                            strip_ansi(&output_preview)
                        }))
                        .into_any_element()
                } else {
                    // No output yet
                    div()
                        .p(px(10.))
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .italic()
                        .child("Waiting for output...")
                        .into_any_element()
                }
            })
            // Action buttons
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .mt(px(8.))
                    // Copy button
                    .child(
                        div()
                            .id(SharedString::from(format!("copy-{}", process_id_for_copy)))
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(theme.border)
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |_this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        output_for_copy.clone(),
                                    ));
                                }),
                            )
                            .child("📋 Copy"),
                    )
                    // Kill button (only if running)
                    .when(is_running, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("kill-{}", process_id_for_kill)))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .bg(rgba(0xf14c4c33)) // error with alpha
                                .text_size(px(10.))
                                .text_color(theme.error)
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.8))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.kill_terminal(&process_id_for_kill, cx);
                                    }),
                                )
                                .child("⛔ Kill"),
                        )
                    })
                    // Close button (only if exited)
                    .when(!is_running, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!(
                                    "close-{}",
                                    process_id_for_close
                                )))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .bg(theme.border)
                                .text_size(px(10.))
                                .text_color(theme.text_muted)
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.8))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.system_executions
                                            .remove_terminal(&process_id_for_close);
                                        cx.notify();
                                    }),
                                )
                                .child("✕ Close"),
                        )
                    }),
            )
            // Input hint for terminals with TerminalView
            .when(
                is_running
                    && self
                        .system_executions
                        .terminal_views
                        .contains_key(&process_id),
                |d| {
                    d.child(
                        div()
                            .mt(px(4.))
                            .text_size(px(10.))
                            .text_color(theme.text_muted)
                            .child("Click terminal to focus, then type directly"),
                    )
                },
            )
            // LLM terminal indicator (no TerminalView)
            .when(
                is_running
                    && !self
                        .system_executions
                        .terminal_views
                        .contains_key(&process_id),
                |d| {
                    d.child(
                        div()
                            .mt(px(8.))
                            .px(px(8.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(rgba(0x0078d411))
                            .text_size(px(10.))
                            .text_color(theme.text_muted)
                            .child("🤖 LLM-controlled terminal"),
                    )
                },
            )
            // Resize handle at bottom of terminal card
            .when(
                self.system_executions
                    .terminal_views
                    .contains_key(&process_id),
                |d| {
                    let process_id_for_resize = process_id.clone();
                    d.child(
                        div()
                            .id(SharedString::from(format!("term-resize-{}", process_id)))
                            .w_full()
                            .h(px(8.))
                            .mt(px(4.))
                            .cursor(gpui::CursorStyle::ResizeUpDown)
                            .rounded(px(2.))
                            .bg(theme.border)
                            .hover(|s| s.bg(rgba(0x0078d466)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    let current_height = this
                                        .system_executions
                                        .terminal_heights
                                        .get(&process_id_for_resize)
                                        .copied()
                                        .unwrap_or(250.0);
                                    this.system_executions.resizing_terminal =
                                        Some(process_id_for_resize.clone());
                                    this.system_executions.resize_start_y =
                                        Some(event.position.y.into());
                                    this.system_executions.resize_start_height =
                                        Some(current_height);
                                    cx.notify();
                                }),
                            ),
                    )
                },
            )
    }

    /// Spawn a new user terminal (without a name)
    #[allow(dead_code)]
    fn spawn_user_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.spawn_user_terminal_with_name(None, window, cx);
    }

    /// Spawn a new user terminal with an optional name
    fn spawn_user_terminal_with_name(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let process_id = self.system_executions.generate_process_id();
        let cwd = std::env::current_dir().ok();

        // Spawn terminal in background
        self.spawn_terminal_internal(
            process_id,
            "$SHELL".to_string(),
            cwd,
            ProcessKind::User,
            name,
            window,
            cx,
        );
    }

    /// Kill a running terminal and remove it from the UI
    fn kill_terminal(&mut self, process_id: &str, cx: &mut Context<Self>) {
        // 1. Mark as exited in the store (SIGKILL = 137)
        self.system_executions
            .store
            .mark_finished(process_id, Some(137));

        // 2. Also remove from UI immediately - user wants kill to close too
        self.system_executions.remove_terminal(process_id);

        // 3. Clean up associated view entities
        self.system_executions.terminal_views.remove(process_id);
        self.system_executions.terminal_inputs.remove(process_id);
        self.system_executions.terminal_heights.remove(process_id);

        // TODO: Actually send signal to PTY process
        // For now we just mark it as finished and remove from UI
        cx.notify();
    }

    /// Spawn a terminal process with interactive input support (for User terminals)
    pub fn spawn_terminal_internal(
        &mut self,
        process_id: String,
        command: String,
        cwd: Option<PathBuf>,
        kind: ProcessKind,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create input state for this terminal
        let pid_for_input = process_id.clone();
        let input_entity =
            cx.new(|cx| InputState::new(window, cx).placeholder("Type command and press Enter..."));

        // Subscribe to Enter key events
        cx.subscribe(&input_entity, move |this, input, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { secondary: false } = event {
                // Get the input value and send to terminal
                let value = input.read(cx).value().to_string();
                if !value.is_empty() {
                    this.send_to_terminal(&pid_for_input, &value, cx);
                    // Note: Input is not auto-cleared. User can select all and type new command.
                }
            }
        })
        .detach();

        // Store the input state
        self.system_executions
            .terminal_inputs
            .insert(process_id.clone(), input_entity);

        // Call the common spawn logic
        self.spawn_terminal_common(process_id, command, cwd, kind, name, cx);
    }

    /// Spawn a terminal process without interactive input (for LLM terminals)
    fn spawn_terminal_internal_no_input(
        &mut self,
        process_id: String,
        command: String,
        cwd: Option<PathBuf>,
        kind: ProcessKind,
        cx: &mut Context<Self>,
    ) {
        // No input state for LLM terminals - no name for LLM terminals
        self.spawn_terminal_common(process_id, command, cwd, kind, None, cx);
    }

    /// Common spawn logic shared between interactive and non-interactive terminals
    fn spawn_terminal_common(
        &mut self,
        process_id: String,
        command: String,
        cwd: Option<PathBuf>,
        kind: ProcessKind,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Create initial snapshot
        let snapshot = ProcessSnapshot {
            process_id: process_id.clone(),
            name,
            kind,
            visible: true,
            output: String::new(),
            exit_code: None,
            started_at_ms: now_ms,
            finished_at_ms: None,
        };
        self.system_executions.store.upsert_process(snapshot);

        // Choose environment based on terminal type
        let env = match kind {
            ProcessKind::User => interactive_env(), // Full terminal for TUI apps
            ProcessKind::Llm => headless_env(),     // Headless for LLM commands
        };

        // Spawn the PTY process with larger size for TUI apps
        let config = PtyConfig {
            command: command.clone(),
            cwd,
            size: portable_pty::PtySize {
                rows: 16,
                cols: 50,
                pixel_width: 0,
                pixel_height: 0,
            },
            env,
        };

        match spawn_pty(config) {
            Ok(spawned) => {
                info!(process_id = %process_id, command = %command, "Terminal spawned");

                // Create alacritty terminal for graphical rendering
                let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
                let event_bridge = TerminalEventBridge(event_tx);
                let term_size = TerminalSize {
                    cols: 50,
                    rows: 16,
                    cell_width: 8.4,
                    cell_height: 17.0,
                };
                let term_config = alacritty_terminal::term::Config::default();
                let term =
                    alacritty_terminal::term::Term::new(term_config, &term_size, event_bridge);
                let term = Arc::new(parking_lot::FairMutex::new(term));
                let term_for_view = term.clone();

                // Create TerminalView entity
                let writer_tx_for_view = spawned.writer_tx.clone();
                let terminal_view =
                    cx.new(|cx| TerminalView::new(term_for_view, writer_tx_for_view, cx));
                self.system_executions
                    .terminal_views
                    .insert(process_id.clone(), terminal_view);

                // Store terminal handle for writing input
                self.system_executions.terminals.insert(
                    process_id.clone(),
                    TerminalHandle {
                        process_id: process_id.clone(),
                        writer_tx: spawned.writer_tx,
                        resize_tx: spawned.resize_tx,
                        term: Some(term.clone()),
                    },
                );

                // Start background task to read output
                let store = self.system_executions.store.clone();
                let pid = process_id.clone();

                // Create VTE processor for the background task
                let term_for_vte = term.clone();
                let process_kind = kind; // Clone kind for async closure

                cx.spawn(async move |this, cx| {
                    let mut output_rx = spawned.output_rx;
                    // Use the processor from alacritty_terminal
                    use alacritty_terminal::vte::ansi::Processor as VteProcessor;
                    let mut vte_processor: VteProcessor = VteProcessor::new();

                    while let Some(event) = output_rx.recv().await {
                        match event {
                            PtyEvent::Output(bytes) => {
                                // Process through VTE for terminal emulation
                                // Use catch_unwind to prevent panics from crashing the app
                                let vte_result = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| {
                                        let mut term_guard = term_for_vte.lock();
                                        for byte in &bytes {
                                            vte_processor.advance(&mut *term_guard, *byte);
                                        }
                                    }),
                                );
                                if let Err(e) = vte_result {
                                    // Log the panic but continue processing
                                    let panic_msg = if let Some(s) = e.downcast_ref::<&str>() {
                                        s.to_string()
                                    } else if let Some(s) = e.downcast_ref::<String>() {
                                        s.clone()
                                    } else {
                                        "Unknown panic".to_string()
                                    };
                                    error!(process_id = %pid, error = %panic_msg, "VTE processing panic (recovered)");
                                }

                                // Also store raw text for tool response
                                let text = String::from_utf8_lossy(&bytes);
                                if let Some(mut snapshot) = store.snapshot(&pid) {
                                    snapshot.output.push_str(&text);
                                    store.upsert_process(snapshot);
                                }

                                // Notify UI to re-render
                                let _ = this.update(cx, |_this, cx| {
                                    cx.notify();
                                });
                            }
                            PtyEvent::Exit(code) => {
                                debug!(process_id = %pid, exit_code = ?code, "Terminal exited");
                                store.mark_finished(&pid, code);

                                // Auto-hide LLM terminals when they exit
                                // User terminals stay visible for the user to review
                                if process_kind == ProcessKind::Llm {
                                    store.set_visible(&pid, false);
                                }

                                let _ = this.update(cx, |_this, cx| {
                                    cx.notify();
                                });
                                break;
                            }
                            PtyEvent::Error(e) => {
                                error!(process_id = %pid, error = %e, "Terminal error");
                                store.set_output(&pid, format!("Error: {}", e));
                                store.mark_finished(&pid, Some(-1));
                                let _ = this.update(cx, |_this, cx| {
                                    cx.notify();
                                });
                                break;
                            }
                        }
                    }
                })
                .detach();
            }
            Err(e) => {
                error!(process_id = %process_id, error = %e, "Failed to spawn terminal");
                self.system_executions
                    .store
                    .set_output(&process_id, format!("Failed to spawn: {}", e));
                self.system_executions
                    .store
                    .mark_finished(&process_id, Some(-1));
            }
        }

        cx.notify();
    }

    /// Send input to a terminal's PTY
    fn send_to_terminal(&mut self, process_id: &str, text: &str, cx: &mut Context<Self>) {
        if let Some(handle) = self.system_executions.terminals.get(process_id) {
            // Write text + newline to PTY
            let input = format!("{}\n", text);
            let _ = handle.writer_tx.send(input.into_bytes());
            debug!(process_id = %process_id, input = %text, "Sent input to terminal");
        }
        cx.notify();
    }

    /// Handle a system execution request from tools
    pub fn handle_exec_request(&mut self, request: SystemExecRequest, cx: &mut Context<Self>) {
        match request {
            SystemExecRequest::ExecuteShell {
                request_id,
                command,
                cwd,
            } => {
                info!(
                    request_id = request_id,
                    command = %command,
                    "Handling ExecuteShell request"
                );

                // Validate the command
                let validation = validate_command(&command);

                // Check YOLO mode from settings
                let yolo_mode = Settings::new(&self.db).yolo_mode();

                // Decide if we need approval
                // TRUE YOLO: if enabled, accept EVERYTHING - no exceptions! 🎲
                let needs_approval = !yolo_mode;

                if needs_approval {
                    // Store pending approval and show dialog
                    info!(
                        request_id = request_id,
                        risk_level = ?validation.risk_level,
                        "Command requires user approval"
                    );
                    self.system_executions.pending_approval = Some(PendingApproval {
                        request_id,
                        command,
                        cwd,
                        validation,
                    });
                    // Make sidebar visible to show approval dialog
                    self.system_executions.visible = true;
                    cx.notify();
                } else {
                    // Auto-approve: execute immediately (no window = no interactive input)
                    info!(
                        request_id = request_id,
                        risk_level = ?validation.risk_level,
                        "Auto-approving command (YOLO mode or low risk)"
                    );
                    self.execute_approved_command(request_id, command, cwd, None, cx);
                }
            }
            SystemExecRequest::KillProcess {
                request_id,
                process_id,
            } => {
                info!(
                    request_id = request_id,
                    process_id = %process_id,
                    "Handling KillProcess request"
                );

                self.kill_terminal(&process_id, cx);
                self.system_executions.store.respond(
                    request_id,
                    SystemExecResponse::Killed {
                        process_id: process_id.clone(),
                    },
                );
            }
        }
    }

    /// Execute an approved command (after user approval or auto-approval)
    /// If window is None, this is an auto-approved command without interactive input
    fn execute_approved_command(
        &mut self,
        request_id: u64,
        command: String,
        cwd: Option<String>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let process_id = self.system_executions.generate_process_id();
        info!(
            request_id = request_id,
            process_id = %process_id,
            command = %command,
            "Executing approved command"
        );

        // Send response so tool gets process_id
        self.system_executions.store.respond(
            request_id,
            SystemExecResponse::Started {
                process_id: process_id.clone(),
            },
        );

        // Spawn the terminal
        let cwd_path = cwd.map(PathBuf::from);
        self.spawn_terminal_internal_no_input(process_id, command, cwd_path, ProcessKind::Llm, cx);

        // Suppress unused warning for window (used for future interactive LLM terminals)
        let _ = window;
    }

    /// Approve the pending command
    pub fn approve_pending_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pending) = self.system_executions.pending_approval.take() {
            info!(request_id = pending.request_id, "User approved command");
            self.execute_approved_command(
                pending.request_id,
                pending.command,
                pending.cwd,
                Some(window),
                cx,
            );
        }
    }

    /// Reject the pending command
    pub fn reject_pending_command(&mut self, cx: &mut Context<Self>) {
        if let Some(pending) = self.system_executions.pending_approval.take() {
            info!(request_id = pending.request_id, "User rejected command");
            self.system_executions.store.respond(
                pending.request_id,
                SystemExecResponse::Error {
                    message: "Command rejected by user".to_string(),
                },
            );
            cx.notify();
        }
    }

    /// Render the approval dialog
    pub(crate) fn render_approval_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let has_pending = self.system_executions.pending_approval.is_some();

        div().when(has_pending, |d| {
            let pending = self.system_executions.pending_approval.as_ref().unwrap();
            let validation = &pending.validation;

            // Risk level colors
            let (risk_bg, risk_border, risk_text) = match validation.risk_level {
                RiskLevel::Low => (rgba(0x4ec9b022), rgba(0x4ec9b066), theme.success),
                RiskLevel::Medium => (rgba(0xdcdcaa22), rgba(0xdcdcaa66), theme.warning),
                RiskLevel::High => (rgba(0xf14c4c22), rgba(0xf14c4c66), theme.error),
            };

            let command = pending.command.clone();
            let cwd = pending.cwd.clone();
            let warnings = validation.warnings.clone();
            let risk_emoji = validation.risk_emoji();
            let risk_desc = validation.risk_description();

            d.absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x00000088))
                .occlude()
                .child(
                    div()
                        .w(px(500.))
                        .max_h(px(600.))
                        .bg(theme.panel_background)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(12.))
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        // Header
                        .child(
                            div()
                                .px(px(20.))
                                .py(px(16.))
                                .border_b_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(div().text_size(px(24.)).child(risk_emoji))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(16.))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(theme.text)
                                                .child("Command Approval Required"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(risk_text)
                                                .child(risk_desc),
                                        ),
                                ),
                        )
                        // Content
                        .child(
                            div()
                                .px(px(20.))
                                .py(px(16.))
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                // Risk indicator
                                .child(
                                    div()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(risk_bg)
                                        .border_1()
                                        .border_color(risk_border)
                                        .text_size(px(12.))
                                        .text_color(risk_text)
                                        .child(format!("{} {}", risk_emoji, risk_desc)),
                                )
                                // Command display
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.text_muted)
                                                .child("Command:"),
                                        )
                                        .child(
                                            div()
                                                .id("approval-cmd-display")
                                                .w_full()
                                                .max_h(px(120.))
                                                .overflow_hidden()
                                                .p(px(12.))
                                                .rounded(px(6.))
                                                .bg(theme.background)
                                                .border_1()
                                                .border_color(theme.border)
                                                .font_family("monospace")
                                                .text_size(px(13.))
                                                .text_color(theme.text)
                                                .child(command),
                                        ),
                                )
                                // Working directory (if specified)
                                .when(cwd.is_some(), |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(theme.text_muted)
                                                    .child("Working Directory:"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(12.))
                                                    .py(px(8.))
                                                    .rounded(px(6.))
                                                    .bg(theme.background)
                                                    .border_1()
                                                    .border_color(theme.border)
                                                    .font_family("monospace")
                                                    .text_size(px(12.))
                                                    .text_color(theme.text_muted)
                                                    .child(cwd.unwrap_or_default()),
                                            ),
                                    )
                                })
                                // Warnings (if any)
                                .when(!warnings.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(theme.error)
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child("⚠️ Security Warnings:"),
                                            )
                                            .children(warnings.iter().map(|w| {
                                                div()
                                                    .px(px(12.))
                                                    .py(px(6.))
                                                    .rounded(px(4.))
                                                    .bg(rgba(0xf14c4c22))
                                                    .text_size(px(12.))
                                                    .text_color(theme.error)
                                                    .child(format!("• {}", w))
                                            })),
                                    )
                                }),
                        )
                        // Action buttons
                        .child(
                            div()
                                .px(px(20.))
                                .py(px(16.))
                                .border_t_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap(px(10.))
                                // Reject button
                                .child(
                                    div()
                                        .id("reject-cmd-btn")
                                        .px(px(16.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(rgba(0xf14c4c33))
                                        .border_1()
                                        .border_color(theme.error)
                                        .text_color(theme.error)
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgba(0xf14c4c55)))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.reject_pending_command(cx);
                                            }),
                                        )
                                        .child("✗ Reject"),
                                )
                                // Approve button
                                .child(
                                    div()
                                        .id("approve-cmd-btn")
                                        .px(px(16.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(theme.success)
                                        .text_color(rgb(0xffffff))
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.9))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.approve_pending_command(window, cx);
                                            }),
                                        )
                                        .child("✓ Approve"),
                                ),
                        ),
                )
        })
    }
}

/// Get a preview of the output (last N lines)
fn get_output_preview(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        output.to_string()
    } else {
        let start = lines.len() - max_lines;
        format!("...\n{}", lines[start..].join("\n"))
    }
}

/// Strip ANSI escape sequences from text for clean display
fn strip_ansi(text: &str) -> String {
    let bytes = text.as_bytes();
    let stripped = strip_ansi_escapes::strip(bytes);
    String::from_utf8_lossy(&stripped).to_string()
}
