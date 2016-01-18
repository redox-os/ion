use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{stdout, Read, Write};
use std::env;
use std::process;
use std::thread;

use super::to_num::ToNum;
use super::set_var;
use super::input_editor::readln;

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

pub fn read(args: &[String], variables: &mut HashMap<String, String>) {
    let mut out = stdout();
    for i in 1..args.len() {
        if let Some(arg_original) = args.get(i) {
            let arg = arg_original.trim();
            print!("{}=", arg);
            if let Err(message) = out.flush() {
                println!("{}: Failed to flush stdout", message);
            }
            if let Some(value_original) = readln() {
                let value = value_original.trim();
                set_var(variables, arg, value);
            }
        }
    }
}

pub fn run(args: &[String], variables: &mut HashMap<String, String>) {
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
                        set_var(variables, "?", &format!("{}", code));
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
