//! Messaging system for Stockpot.
//!
//! This module provides a decoupled event-driven architecture where:
//!
//! - **Agents** publish events via [`EventBridge`] to the [`MessageBus`]
//! - **UIs** (terminal, web, VS Code) subscribe and render events
//! - **Sub-agents** naturally work because they publish to the same bus
//!
//! ## Architecture
//!
//! ```text
//!                     ┌──────────────┐
//!                     │  MessageBus  │
//!                     └──────┬───────┘
//!                            │ broadcast
//!           ┌────────────────┼────────────────┐
//!           ▼                ▼                ▼
//!     ┌──────────┐    ┌──────────┐    ┌──────────┐
//!     │ Terminal │    │  Bridge  │    │  Logger  │
//!     │ Renderer │    │ (NDJSON) │    │(optional)│
//!     └──────────┘    └──────────┘    └──────────┘
//!           ▲                ▲
//!           │ publish        │
//!     ┌─────┴────────────────┴─────┐
//!     │       EventBridge          │
//!     │  (StreamEvent → Message)   │
//!     └────────────┬───────────────┘
//!                  │
//!     ┌────────────┴───────────────┐
//!     │      AgentExecutor         │
//!     │    (main or sub-agent)     │
//!     └────────────────────────────┘
//! ```
//!
//! ## Key Components
//!
//! - [`Message`]: UI-agnostic event types (agent lifecycle, tool calls, text, etc.)
//! - [`MessageBus`]: Broadcast channel for pub/sub
//! - [`EventBridge`]: Converts `StreamEvent` to `Message` and publishes
//! - [`TerminalRenderer`]: Renders messages to terminal with colors/formatting
//!
//! ## Usage
//!
//! ```ignore
//! use stockpot_core::messaging::{MessageBus, EventBridge, TerminalRenderer};
//!
//! let bus = MessageBus::new();
//! let renderer = TerminalRenderer::new();
//!
//! // Subscribe and render messages
//! let mut receiver = bus.subscribe();
//! tokio::spawn(async move {
//!     while let Ok(message) = receiver.recv().await {
//!         let _ = renderer.render(&message);
//!     }
//! });
//!
//! // Create executor with bus
//! let executor = AgentExecutor::new(&db, &registry)
//!     .with_bus(bus.sender());
//!
//! // Execute - events automatically flow through!
//! executor.execute_with_bus(agent, model, prompt, ...).await;
//! ```
//!
//! ## Provides
//!
//! - [`Message`] types for agent-UI communication
//! - [`MessageBus`] for bidirectional messaging
//! - Terminal rendering with syntax highlighting
//! - Animated spinner for activity indication

mod bus;
mod event_bridge;
mod types;

pub use bus::{MessageBus, MessageSender};
pub use event_bridge::EventBridge;
pub use types::*;
