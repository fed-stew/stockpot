//! Event handling for keyboard, mouse, and terminal events

use std::time::Duration;

use arboard::Clipboard;
use crossterm::event::{self, Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::mpsc;

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Terminal key press
    Key(KeyEvent),
    /// Mouse event (click, scroll, drag)
    Mouse(MouseEvent),
    /// Text selection started (mouse down)
    SelectionStart { row: u16, col: u16 },
    /// Text selection updated (mouse drag)
    SelectionUpdate { row: u16, col: u16 },
    /// Text selection ended (mouse up)
    SelectionEnd,
    /// Mouse click
    Click {
        row: u16,
        col: u16,
        button: MouseButton,
    },
    /// Terminal resize
    Resize(u16, u16),
    /// Tick for animations/updates
    Tick,
    /// Agent streaming text
    StreamText(String),
    /// Agent finished
    AgentComplete,
    /// Error occurred
    Error(String),
    /// Clipboard paste
    Paste(String),
}

/// Event handler that polls for terminal events
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    /// Create a new event handler with the given tick rate
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        // Spawn terminal event polling task
        std::thread::spawn(move || {
            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key)) => {
                            if event_tx.send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Mouse(mouse)) => {
                            match mouse.kind {
                                MouseEventKind::Down(btn) => {
                                    if btn == MouseButton::Left {
                                        let _ = event_tx.send(AppEvent::SelectionStart {
                                            row: mouse.row,
                                            col: mouse.column,
                                        });
                                    }
                                    // Also send raw mouse event for compatibility/other handlers
                                    let _ = event_tx.send(AppEvent::Mouse(mouse));
                                }
                                MouseEventKind::Drag(btn) => {
                                    if btn == MouseButton::Left {
                                        let _ = event_tx.send(AppEvent::SelectionUpdate {
                                            row: mouse.row,
                                            col: mouse.column,
                                        });
                                    }
                                }
                                MouseEventKind::Up(btn) => {
                                    if btn == MouseButton::Left {
                                        let _ = event_tx.send(AppEvent::SelectionEnd);
                                        let _ = event_tx.send(AppEvent::Click {
                                            row: mouse.row,
                                            col: mouse.column,
                                            button: btn,
                                        });
                                    }
                                    let _ = event_tx.send(AppEvent::Mouse(mouse));
                                }
                                _ => {
                                    if event_tx.send(AppEvent::Mouse(mouse)).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Ok(Event::Resize(w, h)) => {
                            if event_tx.send(AppEvent::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Paste(text)) => {
                            if event_tx.send(AppEvent::Paste(text)).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Send tick on timeout
                    if event_tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                }
            }
        });

        Self { rx, tx }
    }

    /// Get the sender for external events (streaming, etc.)
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
    }

    /// Receive the next event
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

/// Clipboard manager for copy/paste operations
pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            clipboard: Clipboard::new().ok(),
        }
    }

    /// Copy text to clipboard
    pub fn copy(&mut self, text: &str) -> bool {
        if let Some(ref mut clipboard) = self.clipboard {
            clipboard.set_text(text).is_ok()
        } else {
            false
        }
    }

    /// Paste text from clipboard
    pub fn paste(&mut self) -> Option<String> {
        self.clipboard.as_mut()?.get_text().ok()
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
