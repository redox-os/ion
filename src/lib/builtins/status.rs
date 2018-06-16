use builtins::man_pages::{print_man, MAN_STATUS};
use shell::Shell;

use std::env;

bitflags! {
    struct Flags : u8 {
        const HELP = 1;
        const LOGIN_SHELL = 2;
        const INTERACTIVE = 4;
        const FILENAME = 8;
    }
}

pub(crate) fn status(args: &[String], shell: &mut Shell) -> Result<(), String> {
    let mut flags = Flags::empty();
    let shell_args: Vec<_> = env::args().collect();

    let is_login = shell_args[0].chars().nth(0).unwrap() == '-';

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
            match &**arg {
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
                        _ => (),
                    }
                },
            }
        }
        let err = "".to_string();

        if flags.contains(Flags::LOGIN_SHELL) && !is_login {
            return Err(err);
        }

        if flags.contains(Flags::INTERACTIVE) && shell.is_background_shell || shell.is_library {
            return Err(err);
        }

        if flags.contains(Flags::FILENAME) {
            // TODO: This will not work if ion is renamed.
            let sa_len = shell_args.len() - 1;
            let last_sa = &shell_args[sa_len];
            let last_3: String = last_sa[last_sa.len() - 3..last_sa.len()].to_string();

            if last_3 == "ion" {
                println!("stdio");
            } else {
                println!("{}", last_sa);
            }
        }

        if flags.contains(Flags::HELP) {
            print_man(MAN_STATUS);
        }
    }
    Ok(())
}
