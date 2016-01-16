use std::collections::BTreeMap;
use std::string::String;
use std::vec::Vec;
use std::fs::{self, File};
use std::io::{stdout, Read, Write};
use std::env;
use std::process;
use std::thread;

use super::to_num::ToNum;
use super::set_var;
use super::input_editor::readln;

pub fn cat(args: &[String]) {
    let path = args.get(1).map_or(String::new(), |arg| arg.clone());

    match File::open(&path) {
        Ok(mut file) => {
            let mut string = String::new();
            match file.read_to_string(&mut string) {
                Ok(_) => println!("{}", string),
                Err(err) => println!("Failed to read: {}: {}", path, err),
            }
        }
        Err(err) => println!("Failed to open file: {}: {}", path, err),
    }
}

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

pub fn echo(args: &[String]) {
    let echo = args.iter()
                   .skip(1)
                   .fold(String::new(), |string, arg| string + " " + arg);
    println!("{}", echo.trim());
}

pub fn free() {
    match File::open("memory:") {
        Ok(mut file) => {
            let mut string = String::new();
            match file.read_to_string(&mut string) {
                Ok(_) => println!("{}", string),
                Err(err) => println!("Failed to read: memory: {}", err),
            }
        }
        Err(err) => println!("Failed to open file: memory: {}", err),
    }
}

pub fn ls(args: &[String]) {
    let path = args.get(1).map_or(".".to_string(), |arg| arg.clone());

    let mut entries = Vec::new();
    match fs::read_dir(&path) {
        Ok(dir) => {
            for entry_result in dir {
                match entry_result {
                    Ok(entry) => {
                        let directory = match entry.file_type() {
                            Ok(file_type) => file_type.is_dir(),
                            Err(err) => {
                                println!("Failed to read file type: {}", err);
                                false
                            }
                        };

                        match entry.file_name().to_str() {
                            Some(path_str) => {
                                if directory {
                                    entries.push(path_str.to_string() + "/")
                                } else {
                                    entries.push(path_str.to_string())
                                }
                            }
                            None => println!("Failed to convert path to string"),
                        }
                    }
                    Err(err) => println!("Failed to read entry: {}", err),
                }
            }
        }
        Err(err) => println!("Failed to open directory: {}: {}", path, err),
    }

    entries.sort();

    for entry in entries {
        println!("{}", entry);
    }
}

pub fn mkdir(args: &[String]) {
    match args.get(1) {
        Some(dir_name) => {
            if let Err(err) = fs::create_dir(dir_name) {
                println!("Failed to create: {}: {}", dir_name, err);
            }
        }
        None => println!("No name provided"),
    }
}

pub fn poweroff() {
    match File::create("acpi:off") {
        Err(err) => println!("Failed to remove power (error: {})", err),
        Ok(_) => println!("I see dead people"),
    }
}

pub fn ps() {
    match File::open("context:") {
        Ok(mut file) => {
            let mut string = String::new();
            match file.read_to_string(&mut string) {
                Ok(_) => println!("{}", string),
                Err(err) => println!("Failed to read: context: {}", err),
            }
        }
        Err(err) => println!("Failed to open file: context: {}", err),
    }
}

pub fn pwd() {
    match env::current_dir() {
        Ok(path) => {
            match path.to_str() {
                Some(path_str) => println!("{}", path_str),
                None => println!("?"),
            }
        }
        Err(err) => println!("Failed to get current dir: {}", err),
    }
}

pub fn read(args: &[String], variables: &mut BTreeMap<String, String>) {
    for i in 1..args.len() {
        if let Some(arg_original) = args.get(i) {
            let arg = arg_original.trim();
            print!("{}=", arg);
            if let Err(message) = stdout().flush() {
                println!("{}: Failed to flush stdout", message);
            }
            if let Some(value_original) = readln() {
                let value = value_original.trim();
                set_var(variables, arg, value);
            }
        }
    }
}

pub fn rm(args: &[String]) {
    match args.get(1) {
        Some(path) => {
            if fs::remove_file(path).is_err() {
                println!("Failed to remove: {}", path);
            }
        }
        None => println!("No name provided"),
    }
}

pub fn rmdir(args: &[String]) {
    match args.get(1) {
        Some(path) => {
            if fs::remove_dir(path).is_err() {
                println!("Failed to remove: {}", path);
            }
        }
        None => println!("No name provided"),
    }
}

pub fn run(args: &[String], variables: &mut BTreeMap<String, String>) {
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

pub fn sleep(args: &[String]) {
    let secs = args.get(1).map_or(0, |arg| arg.to_num());
    thread::sleep_ms(secs as u32 * 1000);
}

pub fn touch(args: &[String]) {
    match args.get(1) {
        Some(file_name) => {
            if let Err(err) = File::create(file_name) {
                println!("Failed to create: {}: {}", file_name, err);
            }
        }
        None => println!("No name provided"),
    }
}
