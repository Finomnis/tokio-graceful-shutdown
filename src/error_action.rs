use bytemuck::NoUninit;

/// Possible ways a subsystem can react to errors.
///
/// An error will propagate upwards in the subsystem tree until
/// it reaches a subsystem that won't forward it to its parent.
///
/// If an error reaches the [`Toplevel`](crate::Toplevel), a global shutdown will be initiated.
///
/// Also see:
/// - [`SubsystemBuilder::on_failure`](crate::SubsystemBuilder::on_failure)
/// - [`SubsystemBuilder::on_panic`](crate::SubsystemBuilder::on_panic)
/// - [`NestedSubsystem::change_failure_action`](crate::NestedSubsystem::change_failure_action)
/// - [`NestedSubsystem::change_panic_action`](crate::NestedSubsystem::change_panic_action)
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, NoUninit)]
#[repr(u8)]
pub enum ErrorAction {
    /// Pass the error on to the parent subsystem, but don't react to it.
    Forward,
    /// Store the error so it can be retrieved through
    /// [`NestedSubsystem::join`](crate::NestedSubsystem::join),
    /// then initiate a shutdown of the subsystem and its children.
    /// Do not forward the error to the parent subsystem.
    CatchAndLocalShutdown,
}
