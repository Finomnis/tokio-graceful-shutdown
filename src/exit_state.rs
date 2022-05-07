use crate::{errors::SubsystemError, ErrTypeTraits};

pub struct SubprocessExitState<ErrType: ErrTypeTraits> {
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

pub fn prettify_exit_states<ErrType: ErrTypeTraits>(
    exit_states: &[SubprocessExitState<ErrType>],
) -> Vec<String> {
    let max_subprocess_name_length = exit_states
        .iter()
        .map(|code| code.name.len())
        .max()
        .unwrap_or(0);

    let mut exit_states = exit_states.iter().collect::<Vec<_>>();
    exit_states.sort_by_key(|el| el.name.clone());

    exit_states
        .iter()
        .map(
            |SubprocessExitState {
                 name,
                 exit_state,
                 raw_result: _,
             }| {
                let required_padding_length = max_subprocess_name_length - name.len();
                let padding = " ".repeat(required_padding_length);

                name.clone() + &padding + "  => " + exit_state
            },
        )
        .collect::<Vec<_>>()
}
