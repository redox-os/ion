use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd};

use super::status::{TERMINATED, NO_SUCH_COMMAND};

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
