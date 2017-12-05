use super::super::{Capture, Function, Shell};
use parser::shell_expand::expand_string;
use std::io::Read;
use std::process;
use sys;

pub(crate) fn command_not_found(shell: &mut Shell, command: &str) -> Option<String> {
    let function = shell.functions.get("COMMAND_NOT_FOUND")? as *const Function;

    let mut output = None;

    match shell.fork(Capture::Stdout, |child| unsafe {
        let _ = function.read().execute(child, &["ion", command]);
    }) {
        Ok(result) => {
            let mut string = String::new();
            match result.stdout.unwrap().read_to_string(&mut string) {
                Ok(_) => output = Some(string),
                Err(err) => {
                    eprintln!("ion: error reading stdout of child: {}", err);
                }
            }
        },
        Err(err) => {
            eprintln!("ion: fork error: {}", err);
        }
    }

    // Ensure that the parent retains ownership of the terminal before exiting.
    let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
    output
}
