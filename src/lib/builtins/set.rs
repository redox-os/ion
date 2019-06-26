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
    set [ --help ] [-e | +e] [-x | +x] [-o [vi | emacs]] [- | --] [STRING]...

DESCRIPTION
    Shell options may be set using the '-' character, and unset using the '+' character.

OPTIONS
    -e  Exit immediately if a command exits with a non-zero status.

    -o  Specifies that an argument will follow that sets the key map.
        The keymap argument may be either `vi` or `emacs`.

    -x  Specifies that commands will be printed as they are executed.

    --  Following arguments will be set as positional arguments in the shell.
        If no argument are supplied, arguments will be unset.

    -   Following arguments will be set as positional arguments in the shell.
        If no arguments are suppled, arguments will not be unset."
)]
pub fn set(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--") {
            if arg.len() == 2 {
                positionals = Some(PositionalArgs::UnsetIfNone);
                break;
            }
            return Status::SUCCESS;
        } else if arg.starts_with('-') {
            if arg.len() == 1 {
                positionals = Some(PositionalArgs::RetainIfNone);
                break;
            }
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.opts_mut().err_exit = true,
                    _ => return Status::SUCCESS,
                }
            }
        } else if arg.starts_with('+') {
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.opts_mut().err_exit = false,
                    b'x' => shell.opts_mut().print_comms = false,
                    b'o' => match args_iter.next().map(|s| s as &str) {
                        Some("huponexit") => shell.opts_mut().huponexit = false,
                        Some(_) => {
                            return Status::error("ion: set: invalid option");
                        }
                        None => {
                            return Status::error("ion: set: no option given");
                        }
                    },
                    _ => return Status::SUCCESS,
                }
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
                let arguments: types::Array<types::Function<'_>> =
                    iter::once(command).chain(args_iter.cloned().map(Value::Str)).collect();
                if !(kind == PositionalArgs::RetainIfNone && arguments.len() == 1) {
                    shell.variables_mut().set("args", arguments);
                }
            }
        }
    }

    Status::SUCCESS
}
