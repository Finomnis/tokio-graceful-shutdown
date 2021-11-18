mod shutdown_token;
mod signal_handling;
mod subsystem;
mod toplevel;

pub use subsystem::{AsyncSubsystem, SubsystemHandle};
pub use toplevel::Toplevel;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
