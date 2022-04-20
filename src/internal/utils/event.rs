use tokio::sync::watch;

pub struct Event {
    receiver: watch::Receiver<bool>,
}

pub struct EventTrigger {
    sender: watch::Sender<bool>,
}
impl EventTrigger {
    pub fn set(&self) {
        self.sender.send_replace(true);
    }
}

impl Event {
    pub fn create() -> (Self, EventTrigger) {
        let (sender, receiver) = watch::channel(false);
        (Self { receiver }, EventTrigger { sender })
    }

    // pub fn get(&self) -> bool {
    //     *self.receiver.borrow()
    // }

    pub async fn wait(&self) {
        let mut receiver = self.receiver.clone();
        while !*receiver.borrow_and_update() {
            receiver.changed().await.unwrap();
        }
    }
}
