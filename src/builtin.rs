use std::env;
use std::process;

use super::Shell;

pub fn cd(args: &[String]) {
    match args.get(1) {
        Some(path) => {
            if let Err(err) = env::set_current_dir(&path) {
                println!("Failed to set current dir to {}: {}", path, err);
            }
        }
        None => println!("No path given"),
    }
}

pub fn run(args: &[String], shell: &mut Shell) {
    let path = "/apps/shell/main.bin";

    let mut command = process::Command::new(path);
    for i in 1..args.len() {
        if let Some(arg) = args.get(i) {
            command.arg(arg);
        }
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
