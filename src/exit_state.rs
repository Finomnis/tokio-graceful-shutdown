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
