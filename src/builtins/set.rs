use liner::KeyBindings;
use shell::Shell;
use shell::flags::*;
use std::io::{self, Write};
use std::iter;

const HELP: &'static str = r#"NAME
    set - Set or unset values of shell options and positional parameters.

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
        If no arguments are suppled, arguments will not be unset.
"#;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub(crate) fn set(args: &[&str], shell: &mut Shell) -> i32 {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--") {
            if arg.len() == 2 {
                positionals = Some(UnsetIfNone);
                break;
            }
            if &arg[2..] == "help" {
                let mut stdout = stdout.lock();
                let _ = stdout.write(HELP.as_bytes());
            } else {
                return 0;
            }
        } else if arg.starts_with('-') {
            if arg.len() == 1 {
                positionals = Some(RetainIfNone);
                break;
            }
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.flags |= ERR_EXIT,
                    b'o' => match args_iter.next() {
                        Some(&mode) if mode == "vi" => {
                            if let Some(context) = shell.context.as_mut() {
                                context.key_bindings = KeyBindings::Vi;
                            }
                        }
                        Some(&mode) if mode == "emacs" => {
                            if let Some(context) = shell.context.as_mut() {
                                context.key_bindings = KeyBindings::Emacs;
                            }
                        }
                        Some(_) => {
                            let _ = stderr.lock().write_all(b"set: invalid keymap\n");
                            return 0;
                        }
                        None => {
                            let _ = stderr.lock().write_all(b"set: no keymap given\n");
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
            let arguments = iter::once(command).chain(args_iter.map(|i| i.to_string())).collect();
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
