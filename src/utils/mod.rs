mod wait_forever;
pub use wait_forever::wait_forever;

mod shutdown_guard;
pub use shutdown_guard::ShutdownGuard;

pub fn get_subsystem_name(parent_name: &str, name: &str) -> String {
    match (parent_name, name) {
        ("", "") => "".to_string(),
        (l, "") => l.to_string(),
        ("", r) => r.to_string(),
        (l, r) => l.to_string() + "/" + r,
    }
}
