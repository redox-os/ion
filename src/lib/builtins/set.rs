use liner::KeyBindings;
use shell::Shell;
use shell::flags::*;
use std::iter;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub(crate) fn set(args: &[&str], shell: &mut Shell) -> i32 {
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
                    b'o' => match args_iter.next() {
                        Some(&"vi") => if let Some(context) = shell.context.as_mut() {
                            context.key_bindings = KeyBindings::Vi;
                        },
                        Some(&"emacs") => if let Some(context) = shell.context.as_mut() {
                            context.key_bindings = KeyBindings::Emacs;
                        },
                        Some(&"huponexit") => shell.flags |= HUPONEXIT,
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
                    b'o' => match args_iter.next() {
                        Some(&"huponexit") => shell.flags &= 255 ^ HUPONEXIT,
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
            let command: String = shell.variables.get_array("args").unwrap()[0].to_owned();
            // This used to take a `&[String]` but cloned them all, so although
            // this is non-ideal and could probably be better done with `Rc`, it
            // hasn't got any slower.
            let arguments = iter::once(command)
                .chain(args_iter.map(|i| i.to_string()))
                .collect();
            match kind {
                UnsetIfNone => shell.variables.set_array("args", arguments),
                RetainIfNone => if arguments.len() != 1 {
                    shell.variables.set_array("args", arguments);
                },
            }
        }
    }

    0
}
