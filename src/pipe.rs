use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::fs::File;

use super::status::{TERMINATED, NO_SUCH_COMMAND};
use super::Shell;
use super::peg::Pipeline;

pub fn execute_pipeline(pipeline: Pipeline) -> i32 {
    let mut piped_commands: Vec<Command> = pipeline.jobs.iter().map(|job| { Shell::build_command(job) }).collect();
    if let (Some(stdin_file), Some(command)) = (pipeline.stdin_file, piped_commands.first_mut()) {
        if let Ok(file) = File::open(stdin_file) {
            unsafe { command.stdin(Stdio::from_raw_fd(file.into_raw_fd())); }
        }
    }
    if let Some(stdout_file) = pipeline.stdout_file {
        if let Some(mut command) = piped_commands.last_mut() {
            if let Ok(file) = File::create(stdout_file) {
                unsafe { command.stdout(Stdio::from_raw_fd(file.into_raw_fd())); }
            }
        }
    }
    pipe(&mut piped_commands)
}

/// This function will panic if called with an empty slice
pub fn pipe(commands: &mut [Command]) -> i32 {
    let end = commands.len() - 1;
    for command in &mut commands[..end] {
        command.stdout(Stdio::piped());
    }
    let mut children: Vec<Option<Child>> = vec![];
    for command in commands {
        if let Some(spawned) = children.last() {
            if let Some(ref child) = *spawned {
                if let Some(ref stdout) = child.stdout {
                    unsafe { command.stdin(Stdio::from_raw_fd(stdout.as_raw_fd())); }
                }
            } else {
                command.stdin(Stdio::null());
            }
        }
        children.push(command.spawn().ok());
    }
    wait(&mut children)
}

/// This function will panic if called with an empty vector
fn wait(children: &mut Vec<Option<Child>>) -> i32 {
    let end = children.len() - 1;
    for child in children.drain(..end) {
        if let Some(mut child) = child {
            child.wait();
        }
    }
    if let Some(mut child) = children.pop().unwrap() {
        match child.wait() {
            Ok(status) => {
                if let Some(code) = status.code() {
                    code
                } else {
                    println!("child ended by signal");
                    TERMINATED
                }
            }
            Err(err) => {
                println!("Failed to wait: {}", err);
                100 // TODO what should we return here?
            }
        }
    } else {
        NO_SUCH_COMMAND
    }
}
