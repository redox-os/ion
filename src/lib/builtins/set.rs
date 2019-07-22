use super::Status;
use crate as ion_shell;
use crate::{
    shell::{variables::Value, Shell},
    types,
};
use builtins_proc::builtin;
use std::iter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

#[builtin(
    desc = "Set or unset values of shell options and positional parameters.",
    man = "
SYNOPSIS
    set [ --help ] [-e | +e] [- | --] [STRING]...

DESCRIPTION
    Shell options may be set using the '-' character, and unset using the '+' character.

OPTIONS
    -e  Exit immediately if a command exits with a non-zero status.

    --  Following arguments will be set as positional arguments in the shell.
        If no argument are supplied, arguments will be unset.

    -   Following arguments will be set as positional arguments in the shell.
        If no arguments are suppled, arguments will not be unset.

BASH EQUIVALENTS
    To set the keybindings, see the `keybindings` builtin
    To print commands as they are executed (only with the Ion Shell), see `debug`"
)]
pub fn set(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--" => {
                positionals = Some(PositionalArgs::UnsetIfNone);
                break;
            }
            "-" => {
                positionals = Some(PositionalArgs::RetainIfNone);
                break;
            }
            "-e" => shell.opts_mut().err_exit = true,
            "+e" => shell.opts_mut().err_exit = false,
            _ => {
                return Status::bad_argument(format!(
                    "set: argument '{}' is not recognized. Try adding `--` before it to pass it \
                     as argument to the shell script",
                    arg
                ))
            }
        }
    }

    match positionals {
        None => (),
        Some(kind) => {
            if let Some(Value::Array(array)) = shell.variables().get("args") {
                let command = array[0].clone();
                // This used to take a `&[String]` but cloned them all, so although
                // this is non-ideal and could probably be better done with `Rc`, it
                // hasn't got any slower.
                let arguments: types::Array<_> =
                    iter::once(command).chain(args_iter.cloned().map(Value::Str)).collect();
                if !(kind == PositionalArgs::RetainIfNone && arguments.len() == 1) {
                    shell.variables_mut().set("args", arguments);
                }
            }
        }
    }

    Status::SUCCESS
}
