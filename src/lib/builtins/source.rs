use super::Status;
use crate as ion_shell;
use crate::{shell::Shell, types};
use builtins_proc::builtin;
use std::fs::File;

#[builtin(
    desc = "evaluates given file",
    man = "
SYNOPSIS
    source FILEPATH

DESCRIPTION
    Evaluates the commands in a specified file in the current shell. All changes in shell
    variables will affect the current shell because of this."
)]
pub fn source(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match args.get(1) {
        Some(argument) => {
            if let Ok(file) = File::open(argument.as_str()) {
                if let Err(why) = shell.execute_command(file) {
                    Status::error(format!("ion: {}", why))
                } else {
                    Status::SUCCESS
                }
            } else {
                Status::error(format!("ion: failed to open {}\n", argument))
            }
        }
        None => Status::error("an argument is required for source"),
    }
}
