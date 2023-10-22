use super::*;
use crate::{utils::JoinerToken, BoxedError};

#[test]
fn single_item() {
    let items = RemotelyDroppableItems::new();

    let (count1, _) = JoinerToken::<BoxedError>::new(|_| None);
    assert_eq!(0, count1.count());

    let token1 = items.insert(count1.child_token(|_| None));
    assert_eq!(1, count1.count());

    drop(token1);
    assert_eq!(0, count1.count());
}

#[test]
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
