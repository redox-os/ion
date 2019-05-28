use crate::{
    shell::{variables::Value, Shell},
    types,
};
use small;
use std::iter;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub fn set(args: &[small::String], shell: &mut Shell) -> i32 {
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--") {
            if arg.len() == 2 {
                positionals = Some(UnsetIfNone);
                break;
            }
            return 0;
        } else if arg.starts_with('-') {
            if arg.len() == 1 {
                positionals = Some(RetainIfNone);
                break;
            }
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.opts_mut().err_exit = true,
                    _ => return 0,
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
                            eprintln!("ion: set: invalid option");
                            return 0;
                        }
                        None => {
                            eprintln!("ion: set: no option given");
                            return 0;
                        }
                    },
                    _ => return 0,
                }
            }
        }
    }

    match positionals {
        None => (),
        Some(kind) => {
            let command = shell.variables().get::<types::Array>("args").unwrap()[0].clone();
            // This used to take a `&[String]` but cloned them all, so although
            // this is non-ideal and could probably be better done with `Rc`, it
            // hasn't got any slower.
            let arguments: types::Array =
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

    0
}
