use super::{Capture, Function, Shell};
use std::process;
use sys;

pub(crate) fn command_not_found(shell: &mut Shell, command: &str) -> bool {
    fork_function(shell, "COMMAND_NOT_FOUND", &["ion", command])
}

/// High-level function for executing a function programmatically.
/// NOTE: Always add "ion" as a first argument in `args`.
pub fn fork_function<S: AsRef<str>>(shell: &mut Shell, fn_name: &str, args: &[S]) -> bool {
    let function = match shell.variables.get_function(fn_name) {
        Some(func) => func,
        None => return false,
    };
    let function = &function as *const Function;

    if let Err(err) = shell.fork(Capture::None, |child| {
        let result = unsafe { function.read() }.execute(child, args);
        if let Err(err) = result {
            eprintln!("ion: {} function call: {}", fn_name, err);
        }
    }) {
        eprintln!("ion: fork error: {}", err);
        return false;
    }

    // Ensure that the parent retains ownership of the terminal before exiting.
    let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
    true
}
