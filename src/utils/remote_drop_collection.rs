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

            if let Some(offset) = self.offset.upgrade() {
                let offset = offset.load(Ordering::Acquire);

                if let Some(last_item) = data.pop() {
                    if offset != data.len() {
                        // There must have been at least two items, and we are not at the end.
                        // So swap first before dropping.

                        last_item.offset.store(offset, Ordering::Release);
                        data[offset] = last_item;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{utils::JoinerToken, BoxedError};

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn insert_and_drop() {
        let items = RemotelyDroppableItems::new();

        let (count1, _) = JoinerToken::<BoxedError>::new(|_| None);
        let (count2, _) = JoinerToken::<BoxedError>::new(|_| None);

        assert_eq!(0, count1.count());
        assert_eq!(0, count2.count());

        let _token1 = items.insert(count1.child_token(|_| None));
        assert_eq!(1, count1.count());
        assert_eq!(0, count2.count());

        let _token2 = items.insert(count2.child_token(|_| None));
        assert_eq!(1, count1.count());
        assert_eq!(1, count2.count());

        drop(items);
        assert_eq!(0, count1.count());
        assert_eq!(0, count2.count());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn drop_token() {
        let items = RemotelyDroppableItems::new();

        let (count1, _) = JoinerToken::<BoxedError>::new(|_| None);
        let (count2, _) = JoinerToken::<BoxedError>::new(|_| None);
        let (count3, _) = JoinerToken::<BoxedError>::new(|_| None);
        let (count4, _) = JoinerToken::<BoxedError>::new(|_| None);

        let token1 = items.insert(count1.child_token(|_| None));
        let token2 = items.insert(count2.child_token(|_| None));
        let token3 = items.insert(count3.child_token(|_| None));
        let token4 = items.insert(count4.child_token(|_| None));
        assert_eq!(1, count1.count());
        assert_eq!(1, count2.count());
        assert_eq!(1, count3.count());
        assert_eq!(1, count4.count());

        // Last item
        drop(token4);
        assert_eq!(1, count1.count());
        assert_eq!(1, count2.count());
        assert_eq!(1, count3.count());
        assert_eq!(0, count4.count());

        // Middle item
        drop(token2);
        assert_eq!(1, count1.count());
        assert_eq!(0, count2.count());
        assert_eq!(1, count3.count());
        assert_eq!(0, count4.count());

        // First item
        drop(token1);
        assert_eq!(0, count1.count());
        assert_eq!(0, count2.count());
        assert_eq!(1, count3.count());
        assert_eq!(0, count4.count());

        // Only item
        drop(token3);
        assert_eq!(0, count1.count());
        assert_eq!(0, count2.count());
        assert_eq!(0, count3.count());
        assert_eq!(0, count4.count());
    }
}
