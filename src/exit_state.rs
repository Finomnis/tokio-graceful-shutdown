#[derive(Debug)]
pub struct SubprocessExitState {
    pub name: String,
    pub exit_state: String,
}

impl SubprocessExitState {
    pub fn new(name: &str, exit_state: &str) -> Self {
        Self {
            name: name.to_string(),
            exit_state: exit_state.to_string(),
        }
    }
}

pub type ShutdownResults = Result<Vec<SubprocessExitState>, Vec<SubprocessExitState>>;
