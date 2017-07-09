#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(target_os = "redox")] use syscall;
use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::fs::{File, OpenOptions};
use super::fork::{fork_pipe, create_process_group};
use super::job_control::JobControl;
use super::{JobKind, Shell};
use super::status::*;
use super::signals::{self, SignalHandler};
use parser::peg::{Pipeline, RedirectFrom};
use self::crossplat::*;

/// The `crossplat` module contains components that are meant to be abstracted across
/// different platforms
#[cfg(not(target_os = "redox"))]
pub mod crossplat {
    use nix::{fcntl, unistd};
    use parser::peg::{RedirectFrom};
    use std::fs::File;
    use std::io::Error;
    use std::os::unix::io::{IntoRawFd, FromRawFd};
    use std::process::{Stdio, Command};

    /// When given a process ID, that process's group will be assigned as the foreground process group.
    pub fn set_foreground(pid: u32) {
        let _ = unistd::tcsetpgrp(0, pid as i32);
    }

    pub fn get_pid() -> u32 {
        unistd::getpid() as u32
    }

    /// Set up pipes such that the relevant output of parent is sent to the stdin of child.
    /// The content that is sent depends on `mode`
    pub unsafe fn create_pipe (
        parent: &mut Command,
        child: &mut Command,
        mode: RedirectFrom
    ) -> Result<(), Error> {
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
                let clone = temp_file.try_clone()?;
                // We want to make sure that the temp file we created no longer has ownership
                // over the raw file descriptor otherwise it gets closed
                temp_file.into_raw_fd();
                parent.stdout(Stdio::from_raw_fd(writer));
                parent.stderr(Stdio::from_raw_fd(clone.into_raw_fd()));
            }
        }
        child.stdin(Stdio::from_raw_fd(reader));
        Ok(())
    }
}

#[cfg(target_os = "redox")]
mod crossplat {
    use parser::peg::{RedirectFrom};
    use std::fs::File;
    use std::io;
    use std::os::unix::io::{IntoRawFd, FromRawFd};
    use std::process::{Stdio, Command};
    use syscall;

    pub fn set_foreground(pid: u32) {
        // TODO
    }

    pub fn get_pid() -> u32 {
        syscall::getpid().unwrap() as u32
    }

    #[derive(Debug)]
    pub enum Error {
        Io(io::Error),
        Sys(syscall::Error)
    }

    impl From<io::Error> for Error {
        fn from(data: io::Error) -> Error { Error::Io(data) }
    }

    impl From<syscall::Error> for Error {
        fn from(data: syscall::Error) -> Error { Error::Sys(data) }
    }

    /// Set up pipes such that the relevant output of parent is sent to the stdin of child.
    /// The content that is sent depends on `mode`
    pub unsafe fn create_pipe (
        parent: &mut Command,
        child: &mut Command,
        mode: RedirectFrom
    ) -> Result<(), Error> {
        // XXX: Zero probably is a bad default for this, but `pipe2` will error if it fails, so
        // one could reason that it isn't dangerous.
        let mut fds: [usize; 2] = [0; 2];
        syscall::call::pipe2(&mut fds, syscall::flag::O_CLOEXEC)?;
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
                let clone = temp_file.try_clone()?;
                // We want to make sure that the temp file we created no longer has ownership
                // over the raw file descriptor otherwise it gets closed
                temp_file.into_raw_fd();
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
        // If the given pipeline is a background task, fork the shell.
        if piped_commands[piped_commands.len()-1].1 == JobKind::Background {
            fork_pipe(self, piped_commands)
        } else {
            // While active, the SIGTTOU signal will be ignored.
            let _sig_ignore = SignalHandler::new();
            // Execute each command in the pipeline, giving each command the foreground.
            let exit_status = pipe(self, piped_commands, true);
            // Set the shell as the foreground process again to regain the TTY.
            set_foreground(get_pid());
            exit_status
        }
    }
}

/// This function will panic if called with an empty slice
pub fn pipe (
    shell: &mut Shell,
    commands: Vec<(Command, JobKind)>,
    foreground: bool
) -> i32 {
    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.into_iter();
    loop {
        if let Some((mut parent, mut kind)) = commands.next() {
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
                    // created by crossplat::create_pipe. We purposfully drop the pipes that are
                    // owned by a given command in `wait` in order to close those pipes, sending
                    // EOF to the next command
                    let mut remember = Vec::new();
                    let mut children: Vec<Option<Child>> = Vec::new();

                    macro_rules! spawn_proc {
                        ($cmd:expr) => {{
                            let child = $cmd.before_exec(move || {
                                signals::unblock();
                                create_process_group();
                                Ok(())
                            }).spawn();
                            match child {
                                Ok(child) => {
                                    if foreground { set_foreground(child.id()); }
                                    shell.foreground.push(child.id());
                                    children.push(Some(child))
                                },
                                Err(e) => {
                                    children.push(None);
                                    eprintln! (
                                        "ion: failed to spawn `{}`: {}",
                                        get_command_name($cmd),
                                        e
                                    );
                                }
                            }
                        }};
                    }

                    // Append other jobs until all piped jobs are running
                    while let Some((mut child, ckind)) = commands.next() {
                        if let Err(e) = unsafe {
                            crossplat::create_pipe(&mut parent, &mut child, mode)
                        } {
                            eprintln!("ion: failed to create pipe for redirection: {:?}", e);
                        }
                        spawn_proc!(&mut parent);
                        remember.push(parent);
                        if let JobKind::Pipe(m) = ckind {
                            parent = child;
                            mode = m;
                        } else {
                            // We set the kind to the last child kind that was processed. For
                            // example, the pipeline `foo | bar | baz && zardoz` should have the
                            // previous kind set to `And` after processing the initial pipeline
                            kind = ckind;
                            spawn_proc!(&mut child);
                            remember.push(child);
                            break
                        }
                    }

                    previous_kind = kind;
                    previous_status = wait(shell, &mut children, remember);
                    if previous_status == TERMINATED {
                        terminate_fg(shell);
                        return previous_status;
                    }
                }
                _ => {
                    previous_status = execute_command(shell, &mut parent, foreground);
                    previous_kind = kind;
                }
            }
        } else {
            break
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
    shell.foreground_send(syscall::SIGTERM as i32);
}

fn execute_command(shell: &mut Shell, command: &mut Command, foreground: bool) -> i32 {
    match command.before_exec(move || {
        signals::unblock();
        create_process_group();
        Ok(())
    }).spawn() {
        Ok(child) => {
            if foreground { set_foreground(child.id()); }
            shell.watch_foreground(child.id())
        },
        Err(_) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(command));
            FAILURE
        }
    }
}

/// This function will panic if called with an empty vector
fn wait(shell: &mut Shell, children: &mut Vec<Option<Child>>, commands: Vec<Command>) -> i32 {
    let end = children.len() - 1;
    for entry in children.drain(..end).zip(commands.into_iter()) {
        // _cmd is never used here, but it is important that it gets dropped at the end of this
        // block in order to write EOF to the pipes that it owns.
        if let (Some(child), _cmd) = entry {
            let status = shell.watch_foreground(child.id());
            if status == TERMINATED {
                return status
            }
        }
    }

    if let Some(child) = children.pop().unwrap() {
        shell.watch_foreground(child.id())
    } else {
        NO_SUCH_COMMAND
    }
}

fn get_command_name(command: &Command) -> String {
    format!("{:?}", command).split('"').nth(1).unwrap_or("").to_string()
}
