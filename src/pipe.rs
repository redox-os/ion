use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::fs::{File, OpenOptions};
use std::io::Write;

use super::status::{TERMINATED, NO_SUCH_COMMAND};
use super::peg::{Pipeline,Redirection};

pub fn execute_pipeline(pipeline: Pipeline) -> i32 {
    let mut piped_commands: Vec<Command> = pipeline.jobs
                                                   .iter()
                                                   .map(|job| { job.build_command() })
                                                   .collect();
    let num_commands = piped_commands.len();
    let mut stdin_init = None;
    if let (Some(stdin), Some(command)) = (pipeline.stdin, piped_commands.first_mut()) {
        match stdin {
            Redirection::File { file : f, .. } => {
                match File::open(&f) {
                    Ok(file) => unsafe { command.stdin(Stdio::from_raw_fd(file.into_raw_fd())); },
                    Err(err) => println!("ion: failed to redirect stdin into {}: {}", f, err)
                }
            }
            Redirection::Herestring { literal: l } => {
                command.stdin(Stdio::piped());
                stdin_init = Some(l);
            }
        }
    }
    for mut command in piped_commands.iter_mut().take(num_commands - 1) {
        command.stdout(Stdio::piped());
    }
    if let Some(Redirection::File { file : filename, append }) = pipeline.stdout {
        if let Some(mut command) = piped_commands.last_mut() {
            let file = if append {
                OpenOptions::new().write(true).append(true).open(&filename)
            } else {
                File::create(&filename)
            };
            match file {
                Ok(f) => unsafe { command.stdout(Stdio::from_raw_fd(f.into_raw_fd())); },
                Err(err) => println!("ion: failed to redirect stdout into {}: {}", filename, err)
            }
        }
    }
    let mut children: Vec<Option<Child>> = vec![];
    for mut command in piped_commands {
        if let Some(spawned) = children.last() {
            if let Some(ref child) = *spawned {
                if let Some(ref stdout) = child.stdout {
                    unsafe { command.stdin(Stdio::from_raw_fd(stdout.as_raw_fd())); }
                }
            } else {
                command.stdin(Stdio::null());
            }
        }
        let mut child = command.spawn().ok();
        if child.is_none() {
            println!("ion: command not found: {}", get_command_name(&command));
        }
        if children.last().is_none() && stdin_init.is_some() {
            if let Some(stdin) = child.as_mut().and_then(|ch| ch.stdin.as_mut()) {
                if let Some(init) = stdin_init.take() {
                    if let Err(err) = stdin.write(init.as_bytes()).and_then(|_| stdin.write("\n".as_bytes())) {
                        println!("ion: failed to send input to stdin of first process: {}", err);
                    }
                    if let Err(err) = stdin.flush() {
                        println!("ion: failed to flush stdin of first process: {}", err);
                    }
                }
            }
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
                    println!("Child ended by signal");
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

fn get_command_name(command: &Command) -> String {
    format!("{:?}", command).split('"').nth(1).unwrap_or("").to_string()
}
