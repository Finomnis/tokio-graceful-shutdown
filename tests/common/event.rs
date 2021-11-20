use tokio::sync::watch;

pub struct EventSetter {
    sender: watch::Sender<bool>,
}
pub struct Event {
    receiver: watch::Receiver<bool>,
}

impl Event {
    pub fn create() -> (Self, impl FnOnce() -> ()) {
        let (sender, receiver) = watch::channel(false);
        (Self { receiver }, move || {
            sender.send_replace(true);
        })
    }

    pub fn get(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn wait(&self) {
        let mut receiver = self.receiver.clone();
        while !*receiver.borrow_and_update() {
            receiver.changed().await.unwrap();
        }
    }
}
