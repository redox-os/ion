use super::Status;
use crate as ion_shell;
use crate::{types, Shell};
use builtins_proc::builtin;
use std::io::{self, Write};

#[builtin(
    names = "fn",
    desc = "print a short description of every defined function",
    man = "
SYNOPSIS
    fn [ -h | --help ]

DESCRIPTION
    Prints all the defined functions along with their help, if provided"
)]
pub fn fn_(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();
    let _ = writeln!(stdout, "# Functions");
    for (fn_name, function) in shell.variables().functions() {
        let description = function.description();
        if let Some(ref description) = description {
            let _ = writeln!(stdout, "    {} -- {}", fn_name, description);
        } else {
            let _ = writeln!(stdout, "    {}", fn_name);
        }
    }
    Status::SUCCESS
}
