use std::process;

use super::Shell;

pub fn run<'a, I: IntoIterator<Item=&'a str>>(args: I, shell: &mut Shell) {
    let path = "/apps/shell/main.bin";

    let mut command = process::Command::new(path);
    for arg in args.into_iter().skip(1) {
        command.arg(arg);
    }

    match command.spawn() {
        Ok(mut child) => {
            match child.wait() {
                Ok(status) => {
                    if let Some(code) = status.code() {
                        shell.variables.set_var("?", &format!("{}", code));
                    } else {
                        println!("{}: No child exit code", path);
                    }
                }
                Err(err) => println!("{}: Failed to wait: {}", path, err),
            }
        }
        Err(err) => println!("{}: Failed to execute: {}", path, err),
    }
}
