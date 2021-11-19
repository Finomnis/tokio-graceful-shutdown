#[derive(Debug, Clone)]
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

pub fn join_shutdown_results(
    left: ShutdownResults,
    right: Vec<ShutdownResults>,
) -> ShutdownResults {
    let mut has_error = false;

    let mut result = match left {
        Ok(r) => r,
        Err(r) => {
            has_error = true;
            r
        }
    };

    let right_unwrapped = right
        .into_iter()
        .map(|el| match el {
            Ok(r) => r,
            Err(r) => {
                has_error = true;
                r
            }
        })
        .collect::<Vec<_>>();

    for mut right_element in right_unwrapped {
        result.append(&mut right_element);
    }

    if has_error {
        Err(result)
    } else {
        Ok(result)
    }
}

pub fn prettify_exit_states(exit_codes: &[SubprocessExitState]) -> Vec<String> {
    let max_subprocess_name_length = exit_codes
        .iter()
        .map(|code| code.name.len())
        .max()
        .unwrap_or(0);

    let mut exit_codes = exit_codes.to_vec();
    exit_codes.sort_by_key(|el| el.name.clone());

    exit_codes
        .iter()
        .map(|SubprocessExitState { name, exit_state }| {
            let required_padding_length = max_subprocess_name_length - name.len();
            let padding = " ".repeat(required_padding_length);

            name.clone() + &padding + "  => " + &exit_state
        })
        .collect::<Vec<_>>()
}
