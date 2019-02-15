use liner::KeyBindings;
use shell::{flags::*, Shell};
use small;
use std::iter;
use types;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub(crate) fn set(args: &[small::String], shell: &mut Shell) -> i32 {
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
                    b'e' => shell.flags |= ERR_EXIT,
                    b'o' => match args_iter.next().map(|s| s as &str) {
                        Some("vi") => {
                            if let Some(context) = shell.context.as_mut() {
                                context.key_bindings = KeyBindings::Vi;
                            }
                        }
                        Some("emacs") => {
                            if let Some(context) = shell.context.as_mut() {
                                context.key_bindings = KeyBindings::Emacs;
                            }
                        }
                        Some("huponexit") => shell.flags |= HUPONEXIT,
                        Some(_) => {
                            eprintln!("ion: set: invalid option");
                            return 0;
                        }
                        None => {
                            eprintln!("ion: set: no option given");
                            return 0;
                        }
                    },
                    b'x' => shell.flags |= PRINT_COMMS,
                    _ => return 0,
                }
            }
        } else if arg.starts_with('+') {
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.flags &= 255 ^ ERR_EXIT,
                    b'x' => shell.flags &= 255 ^ PRINT_COMMS,
                    b'o' => match args_iter.next().map(|s| s as &str) {
                        Some("huponexit") => shell.flags &= 255 ^ HUPONEXIT,
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
            let command = shell.variables.get::<types::Array>("args").unwrap()[0].clone();
            // This used to take a `&[String]` but cloned them all, so although
            // this is non-ideal and could probably be better done with `Rc`, it
            // hasn't got any slower.
            let arguments: types::Array = iter::once(command).chain(args_iter.cloned()).collect();
            match kind {
                UnsetIfNone => {
                    shell.variables.set("args", arguments);
                }
                RetainIfNone => {
                    if arguments.len() != 1 {
                        shell.variables.set("args", arguments);
                    }
                }
            }
        }
    }

    0
}
