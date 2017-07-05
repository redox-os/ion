#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(all(unix, not(target_os = "redox")))] use nix::unistd::{fork, ForkResult};
#[cfg(all(unix, not(target_os = "redox")))] use nix::Error as NixError;
#[cfg(target_os = "redox")] use std::error::Error;
use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, IntoRawFd};
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
    // use os_pipe::{pipe, IntoStdio};
    use std::process::{Stdio, Command};
    use std::os::unix::io::{IntoRawFd, FromRawFd};
    use std::io::Error;
    use nix::{fcntl, unistd};
    use std::fs::File;
    use parser::peg::{RedirectFrom};

    // pub unsafe fn handle_piping(parent: &mut Command,
    //                             child: &mut Command,
    //                             mode: RedirectFrom) -> Result<(), Error>
    // {
    //     let (reader, writer) = pipe()?;
    //     match mode {
    //         RedirectFrom::Stdout => {
    //             parent.stdout(writer.into_stdio());
    //         },
    //         RedirectFrom::Stderr => {
    //             parent.stderr(writer.into_stdio());
    //         },
    //         RedirectFrom::Both => {
    //             let writer_clone = writer.try_clone()?;
    //             parent.stdout(writer.into_stdio());
    //             parent.stderr(writer_clone.into_stdio());
    //         }
    //     }
    //     child.stdin(reader.into_stdio());
    //     Ok(())
    // }

    pub unsafe fn handle_piping(parent: &mut Command,
                                child: &mut Command,
                                mode: RedirectFrom) -> Result<(), Error>
    {
        let (reader, writer) = unistd::pipe2(fcntl::O_CLOEXEC)?;
        match mode {
            RedirectFrom::Stdout => {
                parent.stdout(Stdio::from_raw_fd(writer));
            },
            RedirectFrom::Stderr => {
                parent.stderr(Stdio::from_raw_fd(writer));
            },
            RedirectFrom::Both => {
                let temp_file = File::from_raw_fd(writer);
                let clone = temp_file.try_clone()?;  // No short-circuit here!
                parent.stdout(Stdio::from_raw_fd(writer));
                parent.stderr(Stdio::from_raw_fd(clone.into_raw_fd()));
            }
        }
        child.stdin(Stdio::from_raw_fd(reader));
        Ok(())
    }
}

#[cfg(target_os = "redox")]
mod xp {
    use std::io::Error;
    use redox_syscall::{call, flag};

    pub struct Handle;

    pub unsafe fn handle_piping(parent: &mut Command,
                                child: &mut Command,
                                mode: RedirectFrom) -> Result<(), Error>
    {
        // Currently this is "unimplemented" in redox
        let mut fds: [usize; 2] = [-1 as usize, -1 as usize];
        call::pipe2(&mut fds, flag::O_CLOEXEC)?;
        let (reader, writer) = (fds[0], fds[1]);
        match mode {
            RedirectFrom::Stdout => {
                parent.stdout(Stdio::from_raw_fd(writer));
            },
            RedirectFrom::Stderr => {
                parent.stderr(Stdio::from_raw_fd(writer));
            },
            RedirectFrom::Both => {
                let temp_file = File::from_raw_fd(writer);
                let clone = temp_file.try_clone()?;  // No short-circuit here!
                parent.stdout(Stdio::from_raw_fd(writer));
                parent.stderr(Stdio::from_raw_fd(clone.into_raw_fd()));
            }
        }
        child.stdin(Stdio::from_raw_fd(reader));
        Ok(())
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
            fork_pipe(self, piped_commands)
        } else {
            pipe(self, piped_commands)
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

fn fork_pipe(shell: &mut Shell, commands: Vec<(Command, JobKind)>) -> i32 {
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
    let mut commands = commands.into_iter().peekable();
    loop {
        if let Some((mut parent, kind)) = commands.next() {
            // When an `&&` or `||` operator is utilized, execute commands based on the previous status.
            match previous_kind {
                JobKind::And => if previous_status != SUCCESS {
                    if let JobKind::Or = kind { previous_kind = kind }
                    commands.next();
                    continue
                },
                JobKind::Or => if previous_status == SUCCESS {
                    if let JobKind::And = kind { previous_kind = kind }
                    commands.next();
                    continue
                },
                _ => ()
            }

            match kind {
                JobKind::Pipe(mut mode) => {

                    // We need to remember the commands as they own the file descriptors that are
                    // created by xp::handle_piping. We purposfully drop the pipes that are owned
                    // by a given command in `wait` in order to close those pipes, sending EOF to
                    // the next command
                    let mut remember = Vec::new();
                    let mut children: Vec<Option<Child>> = Vec::new();

                    macro_rules! spawn_proc {
                        ($cmd:expr) => {{
                            let child = $cmd.spawn().ok();
                            match child {
                                Some(child) => {
                                    shell.foreground.push(child.id());
                                    children.push(Some(child))
                                },
                                None => {
                                    children.push(None);
                                    eprintln!("ion: command not found: {}", get_command_name($cmd));
                                }
                            }
                        }};
                    }

                    // Append other jobs until all piped jobs are running; this will run for at least
                    // one iteration as we reach this point by matching against a peeked job with
                    // kind == JobKind::Pipe(_)
                    loop {
                        if let Some((mut child, ckind)) = commands.next() {
                            if let Err(e) = unsafe { xp::handle_piping(&mut parent, &mut child, mode)} {
                                eprintln!("ion: failed to create pipe for redirection: {:?}", e);
                            }
                            spawn_proc!(&mut parent);
                            remember.push(parent);
                            if let JobKind::Pipe(m) = ckind {
                                parent = child;
                                mode = m;
                            } else {
                                spawn_proc!(&mut child);
                                remember.push(child);
                                break
                            }
                        } else {
                            eprintln!("ion: expected command to pipe output of `{:?}` into", parent);
                        }
                    }

                    previous_kind = kind;
                    previous_status = wait(shell, &mut children, remember);
                    if previous_status == TERMINATED {
                        shell.foreground_send(libc::SIGTERM);
                        return previous_status;
                    }
                }
                _ => {
                    previous_status = execute_command(shell, &mut parent);
                    previous_kind = kind;
                }
            }
        } else {
            break
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
fn wait(shell: &mut Shell, children: &mut Vec<Option<Child>>, commands: Vec<Command>) -> i32 {
    let end = children.len() - 1;
    for entry in children.drain(..end).zip(commands.into_iter()) {
        // _cmd is never used here, but it is important that it gets dropped at the end of this
        // block in order to write EOF to the pipes that it owns.
        if let (Some(mut child), _cmd) = entry {
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
