mod shutdown_token;
mod signal_handling;
mod submodule_lifetimes;

pub use shutdown_token::{initiate_shutdown, wait_until_shutdown};
pub use signal_handling::register_signal_handlers;
pub use submodule_lifetimes::start_submodule;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
