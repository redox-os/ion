use crate::{builtins::man_pages::MAN_STATUS, shell::Shell};
use small;
use std::env;

pub(crate) fn status(args: &[small::String], shell: &mut Shell) -> Result<(), String> {
    let mut help = false;
    let mut login_shell = false;
    let mut interactive = false;
    let mut filename = false;

    let is_login = env::args().nth(0).unwrap().chars().nth(0).unwrap() == '-';

    match args.len() {
        0 => {
            for arg in args {
                match &**arg {
                    "--help" => help = true,
                    "--is-login" => login_shell = true,
                    "--is-interactive" => interactive = true,
                    "--current-filename" => filename = true,
                    _ => {
                        if arg.starts_with('-') {
                            match arg.chars().nth(1).unwrap() {
                                'h' => help = true,
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
                return Err("".to_string());
            }

            if interactive && shell.is_background_shell || shell.is_library {
                return Err("".to_string());
            }

            if filename {
                // TODO: This will not work if ion is renamed.
                let last_sa = &env::args().last().unwrap();
                let last_3: String = last_sa[last_sa.len() - 3..last_sa.len()].to_string();
                if last_3 == "ion" {
                    println!("stdio");
                } else {
                    println!("{}", last_sa);
                }
            }

            if help {
                println!("{}", MAN_STATUS);
            }
            Ok(())
        }
        1 => {
            if is_login {
                println!("This is a login shell");
            } else {
                println!("This is not a login shell");
            }
            Ok(())
        }
        _ => Err("status takes one argument\n".to_string()),
    }
}
