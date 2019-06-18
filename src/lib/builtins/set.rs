use super::Status;
use crate::{
    shell::{variables::Value, Shell},
    types,
};
use std::iter;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub fn set(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--") {
            if arg.len() == 2 {
                positionals = Some(UnsetIfNone);
                break;
            }
            return Status::SUCCESS;
        } else if arg.starts_with('-') {
            if arg.len() == 1 {
                positionals = Some(RetainIfNone);
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
            if let Some(Value::Array(array)) = shell.variables().get_ref("args") {
                let command = array[0].clone();
                // This used to take a `&[String]` but cloned them all, so although
                // this is non-ideal and could probably be better done with `Rc`, it
                // hasn't got any slower.
                let arguments: types::Array<types::Function<'_>> =
                    iter::once(command).chain(args_iter.cloned().map(Value::Str)).collect();
                match kind {
                    UnsetIfNone => {
                        shell.variables_mut().set("args", arguments);
                    }
                    RetainIfNone => {
                        if arguments.len() != 1 {
                            shell.variables_mut().set("args", arguments);
                        }
                    }
                }
            }
        }
    }

    Status::SUCCESS
}
