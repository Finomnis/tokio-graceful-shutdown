//Clone, Copy, Debug, Eq, PartialEq

use super::*;

#[test]
fn derives() {
    let a = ErrorAction::Forward;
    let b = ErrorAction::CatchAndLocalShutdown;

    assert_ne!(a, b.clone());
    assert_ne!(format!("{a:?}"), format!("{b:?}"));
}
