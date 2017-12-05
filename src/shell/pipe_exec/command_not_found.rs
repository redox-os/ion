use super::super::{Capture, Function, FunctionError, Shell};
use std::process;
use sys;

pub(crate) fn command_not_found(shell: &mut Shell, command: &str) -> bool {
    let function = match shell.functions.get("COMMAND_NOT_FOUND") {
        Some(func) => func as *const Function,
        None => return false
    };

    if let Err(err) = shell.fork(Capture::None, |child| {
        let result = unsafe { function.read() }.execute(child, &["ion", command]);
        if let Err(FunctionError::InvalidArgumentCount) = result {
            eprintln!("ion: COMMAND_NOT_FOUND function takes an invalid number of arguments: must be exactly one");
        }
    }) {
        eprintln!("ion: fork error: {}", err);
        return false;
    }

    // Ensure that the parent retains ownership of the terminal before exiting.
    let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
    true
}
