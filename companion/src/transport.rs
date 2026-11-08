use crate::dedup::EventDeduplicator;
use deadass_shared::GameEvent;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const DEDUP_WINDOW: Duration = Duration::from_millis(200);
const MIN_BROADCAST_BUFFER: usize = 16;

#[derive(Clone, Copy)]
pub struct BusMessage {
    pub event: GameEvent,
}

pub struct EventBus {
    inbound: mpsc::UnboundedSender<GameEvent>,
}

impl EventBus {
    pub fn new(buffer: usize) -> (Self, EventIngress, EventOutlet) {
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let (outbound_tx, _) = broadcast::channel(buffer.max(MIN_BROADCAST_BUFFER));
        let bus = Self {
            inbound: inbound_tx,
        };
        (
            bus,
            EventIngress::new(inbound_rx, outbound_tx.clone()),
            EventOutlet::new(outbound_tx.subscribe()),
        )
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<GameEvent> {
        self.inbound.clone()
    }
}

pub struct EventIngress {
    receiver: mpsc::UnboundedReceiver<GameEvent>,
    broadcaster: broadcast::Sender<BusMessage>,
}

impl EventIngress {
    fn new(
        receiver: mpsc::UnboundedReceiver<GameEvent>,
        broadcaster: broadcast::Sender<BusMessage>,
    ) -> Self {
        Self {
            receiver,
            broadcaster,
        }
    }

    pub async fn run(mut self) {
        let mut dedup = EventDeduplicator::new(DEDUP_WINDOW);
        while let Some(event) = self.receiver.recv().await {
            if !dedup.should_emit(event) {
                continue;
            }
            let _ = self.broadcaster.send(BusMessage { event });
        }
    }
}

pub struct EventOutlet {
    receiver: broadcast::Receiver<BusMessage>,
}

impl EventOutlet {
    fn new(receiver: broadcast::Receiver<BusMessage>) -> Self {
        Self { receiver }
    }

    /// Next event, or `None` once the bus is closed. Falling behind skips the
    /// overwritten events instead of ending the stream.
    pub async fn next(&mut self) -> Option<GameEvent> {
        loop {
            match self.receiver.recv().await {
                Ok(message) => return Some(message.event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "event dispatch fell behind; dropped events");
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadass_shared::EventKind;

    #[tokio::test]
    async fn lagging_outlet_skips_ahead_instead_of_closing() {
        let (sender, receiver) = broadcast::channel(2);
        let mut outlet = EventOutlet::new(receiver);
        for sequence in 0..5 {
            let event = GameEvent::new(sequence, 0, EventKind::Kill);
            assert!(sender.send(BusMessage { event }).is_ok());
        }
        assert_eq!(outlet.next().await.map(|event| event.sequence), Some(3));
        drop(sender);
        assert_eq!(outlet.next().await.map(|event| event.sequence), Some(4));
        assert_eq!(outlet.next().await, None);
    }
}
