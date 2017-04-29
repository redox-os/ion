use std::iter;
use std::io::{self, Write};
use shell::flags::*;
use shell::Shell;

const HELP: &'static str = r#"NAME
    set - Set or unset values of shell options and positional parameters.

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
"#;

enum PositionalArgs {
    UnsetIfNone,
    RetainIfNone,
}

use self::PositionalArgs::*;

pub fn set(args: &[&str], shell: &mut Shell) -> i32 {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut args_iter = args.iter();
    let mut positionals = None;

    while let Some(arg) = args_iter.next() {
        if arg.starts_with("--") {
            if arg.len() == 2 { positionals = Some(UnsetIfNone); break }
            if &arg[2..] == "help" {
                let _ = stdout.write(HELP.as_bytes());
            } else {
                return 0
            }
        } else if arg.starts_with('-') {
            if arg.len() == 1 { positionals = Some(RetainIfNone); break }
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.flags |= ERR_EXIT,
                    _ => {
                        return 0
                    }
                }
            }
        } else if arg.starts_with('+') {
            for flag in arg.bytes().skip(1) {
                match flag {
                    b'e' => shell.flags &= 255 ^ ERR_EXIT,
                    _ => {
                        return 0
                    }
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
                UnsetIfNone  => shell.variables.set_array("args", arguments),
                RetainIfNone => if arguments.len() != 1 {
                    shell.variables.set_array("args", arguments);
                }
            }
        }
    }

    0
}
