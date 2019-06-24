use super::Status;
use crate as ion_shell;
use crate::{shell::Shell, types};
use builtins_proc::builtin;
use std::env;

#[builtin(man = "
Evaluates the current runtime status

SYNOPSIS
    status [ -h | --help ] [-l] [-i]

DESCRIPTION
    With no arguments status displays the current login information of the shell.

OPTIONS
    -l
        returns true if the shell is a login shell. Also --is-login.
    -i
        returns true if the shell is interactive. Also --is-interactive.
    -f
        prints the filename of the currently running script or else stdio. Also --current-filename.
")]
pub fn status(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let mut login_shell = false;
    let mut interactive = false;
    let mut filename = false;

    let is_login = env::args().nth(0).unwrap().chars().nth(0).unwrap() == '-';

    match args.len() {
        0 => {
            for arg in args {
                match &**arg {
                    "--help" => (),
                    "--is-login" => login_shell = true,
                    "--is-interactive" => interactive = true,
                    "--current-filename" => filename = true,
                    _ => {
                        if arg.starts_with('-') {
                            match arg.chars().nth(1).unwrap() {
                                'h' => (),
                                'l' => login_shell = true,
                                'i' => interactive = true,
                                'f' => filename = true,
                                _ => (),
                            }
                        }
                    }
                }
            }

            if login_shell && !is_login {
                return Status::error("");
            }

            if interactive && shell.opts().is_background_shell {
                return Status::error("");
            }

            if filename {
                // TODO: This will not work if ion is renamed.

                let last_sa = &env::args().last().unwrap();
                if last_sa.ends_with("ion") {
                    println!("stdio");
                } else {
                    println!("{}", last_sa);
                }
            }

            Status::SUCCESS
        }
        1 => {
            if is_login {
                println!("This is a login shell");
            } else {
                println!("This is not a login shell");
            }
            Status::SUCCESS
        }
        _ => Status::error("status takes one argument"),
    }
}
