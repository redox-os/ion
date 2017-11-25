use std::env;
use std::error::Error;
use std::io::{Write, stdout};

use shell::Shell;

bitflags! {
    struct Flags : u8 {
        const HELP = 1;
        const LOGIN_SHELL = 2;
        const INTERACTIVE = 4;
        const FILENAME = 8;
    }
}

const MAN_PAGE: &'static str = r#"NAME
    status - Evaluates the current runtime status

SYNOPSIS
    status [ -h | --help ] [-l] [-i]

DESCRIPTION
    With no arguments status displays the current login information of the shell.

OPTIONS
    -l
        returns true if shell is a login shell. Also --is-login.
    -i
        returns true if shell is interactive. Also --is-interactive.
    -f
        prints the filename of the currently running script or stdio. Also --current-filename.
"#; // @MANEND

pub(crate) fn status(args: &[&str], shell: &mut Shell) -> Result<(), String> {
    let mut flags = Flags::empty();
    let shell_args: Vec<_> = env::args().collect();

    let mut is_login = false;
    if shell_args[0].chars().nth(0).unwrap() == '-' {
        is_login = true;
    }

    let args_len = args.len();
    if args_len == 1 {
        if is_login {
            println!("This is a login shell");
        } else {
            println!("This is not a login shell");
        }
    } else if args_len > 2 {
        return Err("status takes one argument\n".to_string());
    } else {
        for arg in args {
            match *arg {
                "--help" => flags |= Flags::HELP,
                "--is-login" => flags |= Flags::LOGIN_SHELL,
                "--is-interactive" => flags |= Flags::INTERACTIVE,
                "--current-filename" => flags |= Flags::FILENAME,
                _ => if arg.starts_with('-') {
                    match arg.chars().nth(1).unwrap() {
                        'h' => flags |= Flags::HELP,
                        'l' => flags |= Flags::LOGIN_SHELL,
                        'i' => flags |= Flags::INTERACTIVE,
                        'f' => flags |= Flags::FILENAME,
                        _ => ()
                    }
                }
            }
        }
        let err = "".to_string();

        if flags.contains(Flags::LOGIN_SHELL) && !is_login {
            return Err(err);
        }

        if flags.contains(Flags::INTERACTIVE) {
            if shell.is_background_shell || shell.is_library {
                return Err(err);
            }
        }

        if flags.contains(Flags::FILENAME) {
            // TODO: This technique will not work if ion is renamed. 
            let sa_len = shell_args.len() - 1;
            let last_sa = &shell_args[sa_len];
            let last_3: String = last_sa[last_sa.len() - 3 .. last_sa.len()].to_string();

            if last_3 == "ion" {
                println!("stdio");
            } else {
                println!("{}", last_sa);
            }
        }

        let stdout = stdout();
        let mut stdout = stdout.lock();

        if flags.contains(Flags::HELP) {
            return match stdout.write_all(MAN_PAGE.as_bytes()).and_then(|_| stdout.flush()) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.description().to_owned()),
            }
        }
    }
    Ok(())
}

