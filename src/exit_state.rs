use crate::{errors::SubsystemError, ErrTypeTraits};

pub struct SubprocessExitState<ErrType: ErrTypeTraits = crate::BoxedError> {
    pub name: String,
    pub exit_state: String,
    pub raw_result: Result<(), SubsystemError<ErrType>>,
}

impl<ErrType: ErrTypeTraits> SubprocessExitState<ErrType> {
    pub fn new(
        name: &str,
        exit_state: &str,
        raw_result: Result<(), SubsystemError<ErrType>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            exit_state: exit_state.to_string(),
            raw_result,
        }
    }
}

pub type ShutdownResults<ErrType> = Vec<SubprocessExitState<ErrType>>;

pub fn join_shutdown_results<ErrType: ErrTypeTraits>(
    mut left: ShutdownResults<ErrType>,
    right: Vec<ShutdownResults<ErrType>>,
) -> ShutdownResults<ErrType> {
    for mut right_element in right {
        left.append(&mut right_element);
    }

    left
}
