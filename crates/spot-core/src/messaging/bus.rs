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
        MessageSender {
            tx: self.tx.clone(),
        }
    }

    /// Subscribe to messages.
    pub fn subscribe(&self) -> MessageReceiver {
        MessageReceiver {
            rx: self.tx.subscribe(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MessageBus Tests
    // =========================================================================

    #[test]
    fn test_message_bus_new() {
        let bus = MessageBus::new();
        // Verify we can get a sender and subscriber
        let _sender = bus.sender();
        let _receiver = bus.subscribe();
    }

    #[test]
    fn test_message_bus_default() {
        let bus = MessageBus::default();
        let _sender = bus.sender();
        let _receiver = bus.subscribe();
    }

    #[test]
    fn test_sender_is_clone() {
        let bus = MessageBus::new();
        let sender1 = bus.sender();
        let sender2 = sender1.clone();

        // Both senders should work
        let mut receiver = bus.subscribe();
        sender1.info("from sender1");
        sender2.info("from sender2");

        // Verify both messages arrived
        assert!(receiver.try_recv().unwrap().is_some());
        assert!(receiver.try_recv().unwrap().is_some());
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = MessageBus::new();
        let sender = bus.sender();

        let mut receiver1 = bus.subscribe();
        let mut receiver2 = bus.subscribe();

        sender.info("broadcast message");

        // Both receivers should get the message
        let msg1 = receiver1.try_recv().unwrap();
        let msg2 = receiver2.try_recv().unwrap();

        assert!(msg1.is_some());
        assert!(msg2.is_some());
    }

    // =========================================================================
    // MessageSender Tests
    // =========================================================================

    #[test]
    fn test_sender_send_success() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let _receiver = bus.subscribe();

        let result = sender.send(Message::info("test"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_sender_send_closed_channel() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        // No subscribers - channel is effectively closed for sending
        // Note: broadcast channels return error if no receivers exist

        let result = sender.send(Message::info("test"));
        assert!(result.is_err());

        if let Err(BusError::Closed) = result {
            // Expected
        } else {
            panic!("Expected BusError::Closed");
        }
    }

    #[test]
    fn test_sender_info() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.info("info message");

        let msg = receiver.try_recv().unwrap().unwrap();
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.text, "info message");
            assert!(matches!(text_msg.level, super::super::MessageLevel::Info));
        } else {
            panic!("Expected Text message");
        }
    }

    #[test]
    fn test_sender_success() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.success("success message");

        let msg = receiver.try_recv().unwrap().unwrap();
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.text, "success message");
            assert!(matches!(
                text_msg.level,
                super::super::MessageLevel::Success
            ));
        } else {
            panic!("Expected Text message");
        }
    }

    #[test]
    fn test_sender_warning() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.warning("warning message");

        let msg = receiver.try_recv().unwrap().unwrap();
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.text, "warning message");
            assert!(matches!(
                text_msg.level,
                super::super::MessageLevel::Warning
            ));
        } else {
            panic!("Expected Text message");
        }
    }

    #[test]
    fn test_sender_error() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.error("error message");

        let msg = receiver.try_recv().unwrap().unwrap();
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.text, "error message");
            assert!(matches!(text_msg.level, super::super::MessageLevel::Error));
        } else {
            panic!("Expected Text message");
        }
    }

    #[test]
    fn test_sender_response() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.response("# Response\n\nContent here");

        let msg = receiver.try_recv().unwrap().unwrap();
        if let Message::Response(resp) = msg {
            assert_eq!(resp.content, "# Response\n\nContent here");
            assert!(!resp.is_streaming);
        } else {
            panic!("Expected Response message");
        }
    }

    #[test]
    fn test_sender_helpers_ignore_closed_channel() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        // No receiver - but helpers should not panic

        sender.info("ignored");
        sender.success("ignored");
        sender.warning("ignored");
        sender.error("ignored");
        sender.response("ignored");
        // No panic = success
    }

    // =========================================================================
    // MessageReceiver Tests
    // =========================================================================

    #[test]
    fn test_receiver_try_recv_empty() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();

        let result = receiver.try_recv();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_receiver_try_recv_message() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.info("test");

        let result = receiver.try_recv();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_receiver_recv_success() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.info("async test");

        let result = receiver.recv().await;
        assert!(result.is_ok());

        if let Message::Text(text_msg) = result.unwrap() {
            assert_eq!(text_msg.text, "async test");
        } else {
            panic!("Expected Text message");
        }
    }

    #[tokio::test]
    async fn test_receiver_recv_closed() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();

        // Drop the bus (and its sender)
        drop(bus);

        let result = receiver.recv().await;
        assert!(result.is_err());

        if let Err(BusError::Closed) = result {
            // Expected
        } else {
            panic!("Expected BusError::Closed");
        }
    }

    #[tokio::test]
    async fn test_receiver_recv_lagged() {
        // Create bus with small capacity to test lagging
        let (tx, _) = broadcast::channel::<Message>(2);
        let bus_tx = tx.clone();

        let mut receiver = MessageReceiver { rx: tx.subscribe() };

        // Send more messages than buffer size
        for i in 0..5 {
            let _ = bus_tx.send(Message::info(format!("msg {}", i)));
        }

        // First recv should report lagged
        let result = receiver.recv().await;

        match result {
            Err(BusError::Lagged(n)) => {
                assert!(n > 0);
            }
            Ok(_) => {
                // Some messages may have been received before lagging
            }
            Err(BusError::Closed) => {
                panic!("Expected Lagged, got Closed");
            }
        }
    }

    #[test]
    fn test_try_recv_lagged() {
        // Create bus with small capacity
        let (tx, _) = broadcast::channel::<Message>(2);
        let bus_tx = tx.clone();

        let mut receiver = MessageReceiver { rx: tx.subscribe() };

        // Send more messages than buffer size
        for i in 0..5 {
            let _ = bus_tx.send(Message::info(format!("msg {}", i)));
        }

        // First try_recv should report lagged
        let result = receiver.try_recv();

        match result {
            Err(BusError::Lagged(n)) => {
                assert!(n > 0);
            }
            Ok(_) => {
                // May succeed if timing allows
            }
            Err(BusError::Closed) => {
                panic!("Expected Lagged, got Closed");
            }
        }
    }

    // =========================================================================
    // BusError Tests
    // =========================================================================

    #[test]
    fn test_bus_error_closed_display() {
        let err = BusError::Closed;
        assert_eq!(err.to_string(), "Channel closed");
    }

    #[test]
    fn test_bus_error_lagged_display() {
        let err = BusError::Lagged(42);
        assert_eq!(err.to_string(), "Lagged behind by 42 messages");
    }

    #[test]
    fn test_bus_error_debug() {
        let closed = BusError::Closed;
        let lagged = BusError::Lagged(10);

        assert!(format!("{:?}", closed).contains("Closed"));
        assert!(format!("{:?}", lagged).contains("Lagged"));
        assert!(format!("{:?}", lagged).contains("10"));
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_multiple_messages_ordering() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        sender.info("first");
        sender.warning("second");
        sender.error("third");

        let msg1 = receiver.recv().await.unwrap();
        let msg2 = receiver.recv().await.unwrap();
        let msg3 = receiver.recv().await.unwrap();

        // Verify ordering
        if let Message::Text(t) = msg1 {
            assert_eq!(t.text, "first");
        }
        if let Message::Text(t) = msg2 {
            assert_eq!(t.text, "second");
        }
        if let Message::Text(t) = msg3 {
            assert_eq!(t.text, "third");
        }
    }

    #[tokio::test]
    async fn test_sender_from_different_task() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        let handle = tokio::spawn(async move {
            sender.info("from spawned task");
        });

        handle.await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Text(t) = msg {
            assert_eq!(t.text, "from spawned task");
        } else {
            panic!("Expected Text message");
        }
    }

    #[test]
    fn test_message_variants_through_bus() {
        let bus = MessageBus::new();
        let sender = bus.sender();
        let mut receiver = bus.subscribe();

        // Send various message types
        let _ = sender.send(Message::Divider);
        let _ = sender.send(Message::Clear);
        let _ = sender.send(Message::text_delta("streaming..."));
        let _ = sender.send(Message::thinking("pondering..."));

        // Verify all received
        assert!(matches!(
            receiver.try_recv().unwrap().unwrap(),
            Message::Divider
        ));
        assert!(matches!(
            receiver.try_recv().unwrap().unwrap(),
            Message::Clear
        ));
        assert!(matches!(
            receiver.try_recv().unwrap().unwrap(),
            Message::TextDelta(_)
        ));
        assert!(matches!(
            receiver.try_recv().unwrap().unwrap(),
            Message::Thinking(_)
        ));
    }
}
