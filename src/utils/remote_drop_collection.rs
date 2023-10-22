use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};

struct RemotelyDroppableItem<T> {
    _item: T,
    offset: Arc<AtomicUsize>,
}

/// A vector that owns a bunch of objects.
/// Every object is connected to a guard token.
/// Once the token is dropped, the object gets dropped as well.
///
/// Note that the token does not keep the object alive, it is only responsible
/// for triggering a drop.
///
/// The important part here is that the token is sendable to other context/threads,
/// so it's basically a 'remote drop guard' concept.
pub(crate) struct RemotelyDroppableItems<T> {
    items: Arc<Mutex<Vec<RemotelyDroppableItem<T>>>>,
}

impl<T> RemotelyDroppableItems<T> {
    pub(crate) fn new() -> Self {
        Self {
            items: Default::default(),
        }
    }

    pub(crate) fn insert(&self, item: T) -> RemoteDrop<T> {
        let mut items = self.items.lock().unwrap();

        let offset = Arc::new(AtomicUsize::new(items.len()));
        let weak_offset = Arc::downgrade(&offset);

        items.push(RemotelyDroppableItem {
            _item: item,
            offset,
        });

        RemoteDrop {
            data: Arc::downgrade(&self.items),
            offset: weak_offset,
        }
    }
}

/// Drops its referenced item when dropped
pub(crate) struct RemoteDrop<T> {
    // Both weak.
    // If data is gone, then our item collection dropped.
    data: Weak<Mutex<Vec<RemotelyDroppableItem<T>>>>,
    // If offset is gone, then the item itself got removed
    // while the dropguard still exists.
    offset: Weak<AtomicUsize>,
}

impl<T> Drop for RemoteDrop<T> {
    fn drop(&mut self) {
        if let Some(data) = self.data.upgrade() {
            // Important: lock first, then read the offset.
            let mut data = data.lock().unwrap();

            self.offset.upgrade().map(|offset| {
                let offset = offset.load(Ordering::Acquire);

                data.pop().map(|last_item| {
                    if offset != data.len() {
                        // There must have been at least two items, and we are not at the end.
                        // So swap first before dropping.

                        last_item.offset.store(offset, Ordering::Release);
                        data[offset] = last_item;
                    }
                });
            });
        }
    }
}

#[cfg(test)]
mod tests;
