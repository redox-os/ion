#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(all(unix, not(target_os = "redox")))] use nix::unistd::{fork, ForkResult};
#[cfg(all(unix, not(target_os = "redox")))] use nix::Error as NixError;
#[cfg(target_os = "redox")] use std::error::Error;
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

/// The `xp` module contains components that are meant to be abstracted across different platforms
#[cfg(not(target_os = "redox"))]
mod xp {
    use nix::{Error, unistd};
    use std::process::{Stdio, Command, Child};
    use std::os::unix::io::{RawFd, FromRawFd, AsRawFd};
    use std::fs::File;
    use std::io::Read;

    #[derive(Debug)]
    pub struct Handle {
        reader: RawFd,
        writer: RawFd
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unistd::close(self.reader).unwrap();
            unistd::close(self.writer).unwrap();
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Ret(Error);

    impl From<Error> for Ret {
        fn from(data: Error) -> Self { Ret(data) }
    }

    pub unsafe fn handle_dual_piping(command: &mut Command, child: &Child) -> Result<Handle, Ret> {
        // The first pipe is for reading and the second pipe is for writing, so we want
        // stdout and stderr to write into this pipe, and stdin to read from this pipe
        // Map both stdio and stderr to the writing end of the pipe
        // Map the reading end of the pipe to stdio for the incoming process
        let (reader, writer) = unistd::pipe()?;
        if let Some(ref stderr) = child.stderr {
            if let Some(ref stdout) = child.stdout {
                unistd::dup2(writer, stderr.as_raw_fd())?;
                unistd::dup2(writer, stdout.as_raw_fd())?;
                command.stdin(Stdio::from_raw_fd(reader));
            }
        }
        Ok(Handle { reader, writer })
    }
}

#[cfg(target_os = "redox")]
mod xp {
    pub struct Ret;
    pub struct Handle;

    pub unsafe fn handle_dual_piping(command: &mut Command, child: &mut Child) -> Result<Handle, Ret> {
        // Currently this is "unimplemented" in redox
        command.stdin(Stdio::from_raw_fd(stderr.as_raw_fd()));
        Ok(Handle{})
    }
}

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
fn ion_fork() -> Result<Fork, Error> {
    use redox_syscall::call::clone;
    unsafe {
        clone(0).map(|pid| if pid == 0 { Fork::Child } else { Fork::Parent(pid as u32)})?
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
fn pipe(shell: &mut Shell, commands: Vec<(Command, JobKind)>) -> i32 {
    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.into_iter().peek();
    while let Some((mut command, kind)) = commands.next() {
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

                macro_rules! spawn_child {
                    ($cmd:expr) => {{
                        let child = $cmd.spawn().ok();
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
                    }};
                }

                let mut handles: Vec<Option<xp::Handle>> = Vec::new();

                let mut producer : &mut Command = command;
                let mut kind = from;

                // Initialize the first job
                match from {
                    RedirectFrom::Both => {
                        command.stderr(Stdio::piped());
                        command.stdout(Stdio::piped());
                    }
                    RedirectFrom::Stderr => { command.stderr(Stdio::piped()); },
                    RedirectFrom::Stdout => { command.stdout(Stdio::piped()); },
                }

                // Append other jobs until all piped jobs are running.
                while let Some(&mut (ref mut command, kind)) = commands.next() {
                    if let JobKind::Pipe(from) = kind {
                        match from {
                            RedirectFrom::Both => {
                                command.stdout(Stdio::piped());
                                command.stderr(Stdio::piped());
                            }
                            RedirectFrom::Stderr => { command.stderr(Stdio::piped()); },
                            RedirectFrom::Stdout => { command.stdout(Stdio::piped()); },
                        };
                    }
                    if let Some(spawned) = children.last() {
                        if let Some(ref child) = *spawned {
                            unsafe {
                                match from {
                                    // TODO: Find a way to properly implement this.
                                    RedirectFrom::Both => {
                                        match xp::handle_dual_piping(command, child) {
                                            Ok(h) => handles.push(Some(h)),
                                            Err(e) => {
                                                eprintln!("ion: failed to pipe stdout and stderr: {:?}", e);
                                                handles.push(None);
                                            }
                                        }
                                    },
                                    RedirectFrom::Stderr => if let Some(ref stderr) = child.stderr {
                                        command.stdin(Stdio::from_raw_fd(stderr.as_raw_fd()));
                                        handles.push(None);
                                    },
                                    RedirectFrom::Stdout => if let Some(ref stdout) = child.stdout {
                                        command.stdin(Stdio::from_raw_fd(stdout.as_raw_fd()));
                                        handles.push(None);
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
                let mut handles: Vec<_> = handles.into_iter().rev().collect();
                previous_status = wait(shell, &mut children, &mut handles);
                if previous_status == TERMINATED {
                    shell.foreground_send(libc::SIGTERM);
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
                    if signal == libc::SIGTSTP {
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
fn wait(shell: &mut Shell, children: &mut Vec<Option<Child>>, handles: &mut Vec<Option<xp::Handle>>) -> i32 {
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
                            if signal == libc::SIGTSTP {
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
        if let Some(handle) = handles.pop() {
            println!("DROPPED");
            drop(handle);
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
                        if signal == libc::SIGTSTP {
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
