//! Message bus for bidirectional communication.

use super::Message;
use tokio::sync::broadcast;

/// Sender half of the message bus.
#[derive(Clone)]
pub struct MessageSender {
    tx: broadcast::Sender<Message>,
}

impl MessageSender {
    /// Send a message.
    pub fn send(&self, message: Message) -> Result<(), BusError> {
        self.tx.send(message).map_err(|_| BusError::Closed)?;
        Ok(())
    }

    /// Send an info message.
    pub fn info(&self, text: impl Into<String>) {
        let _ = self.send(Message::info(text));
    }

    /// Send a success message.
    pub fn success(&self, text: impl Into<String>) {
        let _ = self.send(Message::success(text));
    }

    /// Send a warning message.
    pub fn warning(&self, text: impl Into<String>) {
        let _ = self.send(Message::warning(text));
    }

    /// Send an error message.
    pub fn error(&self, text: impl Into<String>) {
        let _ = self.send(Message::error(text));
    }

    /// Send a response message.
    pub fn response(&self, content: impl Into<String>) {
        let _ = self.send(Message::response(content));
    }
}

/// Receiver half of the message bus.
pub struct MessageReceiver {
    rx: broadcast::Receiver<Message>,
}

impl MessageReceiver {
    /// Receive the next message.
    pub async fn recv(&mut self) -> Result<Message, BusError> {
        self.rx.recv().await.map_err(|e| match e {
            broadcast::error::RecvError::Closed => BusError::Closed,
            broadcast::error::RecvError::Lagged(n) => BusError::Lagged(n),
        })
    }

    /// Try to receive a message without waiting.
    pub fn try_recv(&mut self) -> Result<Option<Message>, BusError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => Err(BusError::Closed),
            Err(broadcast::error::TryRecvError::Lagged(n)) => Err(BusError::Lagged(n)),
        }
    }
}

/// Message bus for agent-UI communication.
pub struct MessageBus {
    tx: broadcast::Sender<Message>,
}

impl MessageBus {
    /// Create a new message bus.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Get a sender.
    pub fn sender(&self) -> MessageSender {
        MessageSender { tx: self.tx.clone() }
    }

    /// Subscribe to messages.
    pub fn subscribe(&self) -> MessageReceiver {
        MessageReceiver { rx: self.tx.subscribe() }
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Bus errors.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("Channel closed")]
    Closed,
    #[error("Lagged behind by {0} messages")]
    Lagged(u64),
}
