#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(all(unix, not(target_os = "redox")))] use nix::unistd::{fork, ForkResult};
#[cfg(all(unix, not(target_os = "redox")))] use nix::Error as NixError;
#[cfg(target_os = "redox")] use syscall;
use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::fs::{File, OpenOptions};
use std::process::exit;
use std::thread;
use std::time::Duration;
use super::job_control::{JobControl, ProcessState};
use super::{JobKind, Shell};
use super::status::*;
use parser::peg::{Pipeline, RedirectFrom};

pub trait PipelineExecution {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32;
}

impl<'a> PipelineExecution for Shell<'a> {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32 {
        // Generate a list of commands from the given pipeline
        let mut piped_commands: Vec<(Command, JobKind)> = pipeline.jobs
            .drain(..).map(|mut job| {
                (job.build_command(), job.kind)
            }).collect();

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
                    OpenOptions::new().create(true).write(true).append(true).open(&stdout.file)
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

        self.foreground.clear();
        if piped_commands[piped_commands.len()-1].1 == JobKind::Background {
            fork_pipe(self, &mut piped_commands)
        } else {
            pipe(self, &mut piped_commands)
        }
    }
}

enum Fork {
    Parent(u32),
    Child
}

#[cfg(target_os = "redox")]
fn ion_fork() -> syscall::error::Result<Fork> {
    use syscall::call::clone;
    unsafe {
        syscall::call::clone(0).map(|pid| {
             if pid == 0 { Fork::Child } else { Fork::Parent(pid as u32) }
        })
    }
}

#[cfg(all(unix, not(target_os = "redox")))]
fn ion_fork() -> Result<Fork, NixError> {
    match fork()? {
        ForkResult::Parent{ child: pid }  => Ok(Fork::Parent(pid as u32)),
        ForkResult::Child                 => Ok(Fork::Child)
    }
}

fn fork_pipe(shell: &mut Shell, commands: &mut [(Command, JobKind)]) -> i32 {
    match ion_fork() {
        Ok(Fork::Parent(pid)) => {
            shell.send_child_to_background(pid, ProcessState::Running);
            SUCCESS
        },
        Ok(Fork::Child) => {
            exit(pipe(shell, commands));
        },
        Err(why) => {
            eprintln!("ion: background job: {}", why);
            FAILURE
        }
    }
}

/// This function will panic if called with an empty slice
fn pipe(shell: &mut Shell, commands: &mut [(Command, JobKind)]) -> i32 {
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
            JobKind::Pipe(mut from) => {
                let mut children: Vec<Option<Child>> = Vec::new();

                // Initialize the first job
                let _ = match from {
                    RedirectFrom::Both | RedirectFrom::Stderr => command.stderr(Stdio::piped()), // TODO: Fix this
                    RedirectFrom::Stdout => command.stdout(Stdio::piped()),
                };

                let child = command.spawn().ok();
                match child {
                    Some(child) => {
                        shell.foreground.push(child.id());
                        children.push(Some(child))
                    },
                    None => {
                        children.push(None);
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(command));
                    }
                }

                // Append other jobs until all piped jobs are running.
                while let Some(&mut (ref mut command, kind)) = commands.next() {
                    if let JobKind::Pipe(from) = kind {
                        let _ = match from {
                            RedirectFrom::Both | RedirectFrom::Stderr => command.stderr(Stdio::piped()), // TODO: Fix this
                            RedirectFrom::Stdout => command.stdout(Stdio::piped()),
                        };
                    }
                    if let Some(spawned) = children.last() {
                        if let Some(ref child) = *spawned {
                            unsafe {
                                match from {
                                    // TODO: Find a way to properly implement this.
                                    RedirectFrom::Both => if let Some(ref stderr) = child.stderr {
                                        command.stdin(Stdio::from_raw_fd(stderr.as_raw_fd()));
                                    },
                                    RedirectFrom::Stderr => if let Some(ref stderr) = child.stderr {
                                        command.stdin(Stdio::from_raw_fd(stderr.as_raw_fd()));
                                    },
                                    RedirectFrom::Stdout => if let Some(ref stdout) = child.stdout {
                                        command.stdin(Stdio::from_raw_fd(stdout.as_raw_fd()));
                                    }
                                }
                            }
                        } else {
                            // The previous command failed to spawn
                            command.stdin(Stdio::null());
                        }
                    }
                    let child = command.spawn().ok();
                    match child {
                        Some(child) => {
                            shell.foreground.push(child.id());
                            children.push(Some(child));
                        },
                        None => {
                            children.push(None);
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(command));
                        }
                    }

                    if let JobKind::Pipe(next) = kind {
                        from = next;
                        continue
                    } else {
                        previous_kind = kind;
                        break
                    }
                }
                previous_status = wait(shell, &mut children);
                if previous_status == TERMINATED {
                    terminate_fg(shell);
                    return previous_status;
                }
            }
            _ => {
                previous_status = execute_command(shell, command);
                previous_kind = kind;
            }
        }
    }

    previous_status
}

#[cfg(all(unix, not(target_os = "redox")))]
fn terminate_fg(shell: &mut Shell) {
    shell.foreground_send(libc::SIGTERM);
}

#[cfg(target_os = "redox")]
fn terminate_fg(shell: &mut Shell) {
    // TODO: Redox does not support signals
}

#[cfg(all(unix, not(target_os = "redox")))]
fn is_sigtstp(signal: i32) -> bool {
    signal == libc::SIGTSTP
}

#[cfg(target_os = "redox")]
fn is_sigtstp(_: i32) -> bool {
    // TODO: Redox does not support signals
    false
}

fn execute_command(shell: &mut Shell, command: &mut Command) -> i32 {
    match command.spawn() {
        Ok(child) => wait_on_child(shell, child),
        Err(_) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(command));
            FAILURE
        }
    }
}

fn wait_on_child(shell: &mut Shell, mut child: Child) -> i32 {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(code) = status.code() {
                    break code
                } else {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = stderr.write_all(b"ion: child ended by signal\n");
                    break TERMINATED
                }
            },
            Ok(None) => {
                if let Ok(signal) = shell.signals.try_recv() {
                    if is_sigtstp(signal) {
                        shell.received_sigtstp = true;
                        let pid = child.id();
                        shell.suspend(pid);
                        shell.send_child_to_background(pid, ProcessState::Stopped);
                        break SUCCESS
                    } else {
                        if let Err(why) = child.kill() {
                            let stderr = io::stderr();
                            let _ = writeln!(stderr.lock(), "ion: unable to kill child: {}", why);
                        }
                        shell.foreground_send(signal);
                        shell.handle_signal(signal);
                        break TERMINATED
                    }
                }
                thread::sleep(Duration::from_millis(1));
            },
            Err(err) => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                break 100 // TODO what should we return here?
            }
        }
    }
}

/// This function will panic if called with an empty vector
fn wait(shell: &mut Shell, children: &mut Vec<Option<Child>>) -> i32 {
    let end = children.len() - 1;
    for child in children.drain(..end) {
        if let Some(mut child) = child {
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if let Some(code) = status.code() {
                            break code
                        } else {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = stderr.write_all(b"ion: child ended by signal\n");
                            break TERMINATED
                        }
                    },
                    Ok(None) => {
                        if let Ok(signal) = shell.signals.try_recv() {
                            if is_sigtstp(signal) {
                                shell.received_sigtstp = true;
                                let pid = child.id();
                                shell.suspend(pid);
                                shell.send_child_to_background(pid, ProcessState::Stopped);
                                break SUCCESS
                            }
                            shell.foreground_send(signal);
                            shell.handle_signal(signal);
                            break TERMINATED
                        }
                        thread::sleep(Duration::from_millis(1));
                    },
                    Err(err) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                        break 100 // TODO what should we return here?
                    }
                }
            };
            if status == TERMINATED {
                return status
            }
        }
    }

    if let Some(mut child) = children.pop().unwrap() {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if let Some(code) = status.code() {
                        break code
                    } else {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = stderr.write_all(b"ion: child ended by signal\n");
                        break TERMINATED
                    }
                },
                Ok(None) => {
                    if let Ok(signal) = shell.signals.try_recv() {
                        if is_sigtstp(signal) {
                            shell.received_sigtstp = true;
                            let pid = child.id();
                            shell.suspend(pid);
                            shell.send_child_to_background(pid, ProcessState::Stopped);
                            break SUCCESS
                        }
                        shell.foreground_send(signal);
                        shell.handle_signal(signal);
                        break TERMINATED
                    }
                    thread::sleep(Duration::from_millis(1));
                },
                Err(err) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                    break 100 // TODO what should we return here?
                }
            }
        }
    } else {
        NO_SUCH_COMMAND
    }
}

fn get_command_name(command: &Command) -> String {
    format!("{:?}", command).split('"').nth(1).unwrap_or("").to_string()
}
