use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::fs::{File, OpenOptions};
use std::thread;

use shell::JobKind;
use status::*;
use parser::peg::{Pipeline, RedirectFrom};

pub fn execute_pipeline(pipeline: &mut Pipeline) -> i32 {
    // Generate a list of commands from the given pipeline
    let mut piped_commands: Vec<(Command, JobKind)> = pipeline.jobs
        .iter().map(|job| (job.build_command(), job.kind)).collect();

    if let Some(ref stdin) = pipeline.stdin {
        if let Some(command) = piped_commands.first_mut() {
            match File::open(&stdin.file) {
                Ok(file) => unsafe { command.0.stdin(Stdio::from_raw_fd(file.into_raw_fd())); },
                Err(err) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: failed to redirect stdin into {}: {}", stdin.file, err);
                }
            }
        }
    }

    if let Some(ref stdout) = pipeline.stdout {
        if let Some(mut command) = piped_commands.last_mut() {
            let file = if stdout.append {
                OpenOptions::new().write(true).append(true).open(&stdout.file)
            } else {
                File::create(&stdout.file)
            };
            match file {
                Ok(f) => unsafe {
                    match stdout.from {
                        RedirectFrom::Both => {
                            let fd = f.into_raw_fd();
                            command.0.stderr(Stdio::from_raw_fd(fd));
                            command.0.stdout(Stdio::from_raw_fd(fd));
                        },
                        RedirectFrom::Stderr => {
                            command.0.stderr(Stdio::from_raw_fd(f.into_raw_fd()));
                        },
                        RedirectFrom::Stdout => {
                            command.0.stdout(Stdio::from_raw_fd(f.into_raw_fd()));
                        },
                    }
                },
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
pub fn pipe(commands: &mut [(Command, JobKind)]) -> i32 {
    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.iter_mut();
    while let Some(&mut (ref mut command, kind)) = commands.next() {
        // When an `&&` or `||` operator is utilized, execute commands based on the previous status.
        match previous_kind {
            JobKind::And => if previous_status != SUCCESS {
                if let JobKind::Or = kind { previous_kind = kind }
                continue
            },
            JobKind::Or => if previous_status == SUCCESS {
                if let JobKind::And = kind { previous_kind = kind }
                continue
            },
            _ => ()
        }

        match kind {
            JobKind::Background => {
                match command.spawn() {
                    Ok(child) => {
                        let _ = thread::spawn(move || {
                            // TODO: Implement proper backgrounding support
                            let status = wait_on_child(child);
                            println!("ion: background task completed: {}", status);
                        });
                    },
                    Err(_) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(&command));
                    }
                }
            },
            JobKind::Pipe => {
                let mut children: Vec<Option<Child>> = Vec::new();

                // Initialize the first job
                command.stdout(Stdio::piped());
                let child = command.spawn().ok();
                if child.is_none() {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(&command));
                }
                children.push(child);

                // Append other jobs until all piped jobs are running.
                while let Some(&mut (ref mut command, kind)) = commands.next() {
                    if let JobKind::Pipe = kind { command.stdout(Stdio::piped()); }
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

                    if let JobKind::Pipe = kind { continue } else { previous_kind = kind; break}
                }
                previous_status = wait(&mut children);
            }
            _ => {
                previous_status = execute_command(command);
                previous_kind = kind;
            }
        }
    }
    previous_status
}

fn execute_command(command: &mut Command) -> i32 {
    match command.spawn() {
        Ok(child) => wait_on_child(child),
        Err(_) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(&command));
            FAILURE
        }
    }
}

fn wait_on_child(mut child: Child) -> i32 {
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
