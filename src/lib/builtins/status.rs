use crate::{
    builtins::man_pages::MAN_STATUS,
    shell::{status::Status, Shell},
};
use small;
use std::env;

pub fn status(args: &[small::String], shell: &mut Shell<'_>) -> Status {
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

            if help {
                println!("{}", MAN_STATUS);
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
