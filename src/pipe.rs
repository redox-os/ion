use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::fs::{File, OpenOptions};

use status::{TERMINATED, NO_SUCH_COMMAND};
use parser::peg::Pipeline;

pub fn execute_pipeline(pipeline: Pipeline) -> i32 {
    let mut piped_commands: Vec<Command> = pipeline.jobs
                                                   .iter()
                                                   .map(|job| { job.build_command() })
                                                   .collect();
    if let (Some(stdin), Some(command)) = (pipeline.stdin, piped_commands.first_mut()) {
        match File::open(&stdin.file) {
            Ok(file) => unsafe { command.stdin(Stdio::from_raw_fd(file.into_raw_fd())); },
            Err(err) => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: failed to redirect stdin into {}: {}", stdin.file, err);
            }
        }
    }
    if let Some(stdout) = pipeline.stdout {
        if let Some(mut command) = piped_commands.last_mut() {
            let file = if stdout.append {
                OpenOptions::new().write(true).append(true).open(&stdout.file)
            } else {
                File::create(&stdout.file)
            };
            match file {
                Ok(f) => unsafe { command.stdout(Stdio::from_raw_fd(f.into_raw_fd())); },
                Err(err) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: failed to redirect stdout into {}: {}", stdout.file, err);
                }
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
                // The previous command failed to spawn
                command.stdin(Stdio::null());
            }
        }
        let child = command.spawn().ok();
        if child.is_none() {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(&command));
        }
        children.push(child);
    }
    wait(&mut children)
}

/// This function will panic if called with an empty vector
fn wait(children: &mut Vec<Option<Child>>) -> i32 {
    let end = children.len() - 1;
    for child in children.drain(..end) {
        if let Some(mut child) = child {
            let _ = child.wait();
        }
    }
    if let Some(mut child) = children.pop().unwrap() {
        match child.wait() {
            Ok(status) => {
                if let Some(code) = status.code() {
                    code
                } else {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = stderr.write_all(b"ion: child ended by signal\n");
                    TERMINATED
                }
            }
            Err(err) => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                100 // TODO what should we return here?
            }
        }
    } else {
        NO_SUCH_COMMAND
    }
}

fn get_command_name(command: &Command) -> String {
    format!("{:?}", command).split('"').nth(1).unwrap_or("").to_string()
}
