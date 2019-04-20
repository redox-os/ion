use crate::{
    shell::{variables::Value, Capture, Shell},
    sys,
};
use std::process;

pub(crate) fn command_not_found(shell: &mut Shell, command: &str) -> bool {
    fork_function(shell, "COMMAND_NOT_FOUND", &["ion", command])
}

/// High-level function for executing a function programmatically.
/// NOTE: Always add "ion" as a first argument in `args`.
pub fn fork_function<S: AsRef<str>>(shell: &mut Shell, fn_name: &str, args: &[S]) -> bool {
    if let Some(Value::Function(function)) = shell.variables.get_ref(fn_name) {
        if let Err(err) = shell.fork(Capture::None, |child| {
            if let Err(err) = function.execute(child, args) {
                eprintln!("ion: {} function call: {}", fn_name, err);
            }
        }) {
            eprintln!("ion: fork error: {}", err);
            false
        } else {
            // Ensure that the parent retains ownership of the terminal before exiting.
            let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
            true
        }
    } else {
        false
    }
}
