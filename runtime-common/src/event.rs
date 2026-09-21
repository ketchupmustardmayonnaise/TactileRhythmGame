use crossbeam_channel::{Receiver, Sender, unbounded};

use sdk::event::Event;

/// 이벤트 채널 시스템
pub struct EventChannel {
    receiver: Receiver<Event>,
    sender: Sender<Event>,
}

impl Default for EventChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl EventChannel {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        Self { receiver, sender }
    }

    pub fn sender(&self) -> Sender<Event> {
        self.sender.clone()
    }

    pub fn push(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    pub fn pop(&self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }
}
