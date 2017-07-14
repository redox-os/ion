#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(target_os = "redox")] use syscall;
use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::fs::{File, OpenOptions};
use super::flags::*;
use super::fork::{fork_pipe, create_process_group};
use super::job_control::JobControl;
use super::{JobKind, Shell};
use super::status::*;
use super::signals::{self, SignalHandler};
use parser::peg::{Pipeline, Input, RedirectFrom};
use self::crossplat::*;

/// The `crossplat` module contains components that are meant to be abstracted across
/// different platforms
#[cfg(not(target_os = "redox"))]
pub mod crossplat {
    use nix::{fcntl, unistd};
    use parser::peg::{RedirectFrom};
    use std::fs::File;
    use std::io::{Write, Error};
    use std::os::unix::io::{IntoRawFd, FromRawFd};
    use std::process::{Stdio, Command};

    /// When given a process ID, that process's group will be assigned as the foreground process group.
    pub fn set_foreground(pid: u32) {
        let _ = unistd::tcsetpgrp(0, pid as i32);
    }

    pub fn get_pid() -> u32 {
        unistd::getpid() as u32
    }

    /// Create an instance of Stdio from a byte slice that will echo the
    /// contents of the slice when read. This can be called with owned or
    /// borrowed strings
    pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<Stdio, Error> {
        let (reader, writer) = unistd::pipe2(fcntl::O_CLOEXEC)?;
        let mut infile = File::from_raw_fd(writer);
        // Write the contents; make sure to use write_all so that we block until
        // the entire string is written
        infile.write_all(input.as_ref())?;
        infile.flush()?;
        // `infile` currently owns the writer end RawFd. If we just return the reader end
        // and let `infile` go out of scope, it will be closed, sending EOF to the reader!
        Ok(Stdio::from_raw_fd(reader))
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
pub mod crossplat {
    use parser::peg::{RedirectFrom};
    use std::fs::File;
    use std::io::{self, Error, Write};
    use std::os::unix::io::{IntoRawFd, FromRawFd};
    use std::process::{Stdio, Command};
    use syscall;

    pub fn set_foreground(pid: u32) {
        // TODO
    }

    pub fn get_pid() -> u32 {
        syscall::getpid().unwrap() as u32
    }

    pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<Stdio, Error> {
        let mut fds: [usize; 2] = [0; 2];
        syscall::call::pipe2(&mut fds, syscall::flag::O_CLOEXEC)
                      .map_err(|e| Error::from_raw_os_error(e.errno))?;
        let (reader, writer) = (fds[0], fds[1]);
        let mut infile = File::from_raw_fd(writer);
        // Write the contents; make sure to use write_all so that we block until
        // the entire string is written
        infile.write_all(input.as_ref())?;
        infile.flush()?;
        // `infile` currently owns the writer end RawFd. If we just return the reader end
        // and let `infile` go out of scope, it will be closed, sending EOF to the reader!
        Ok(Stdio::from_raw_fd(reader))
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
        syscall::call::pipe2(&mut fds, syscall::flag::O_CLOEXEC)
                      .map_err(|e| Error::from_raw_os_error(e.errno))?;
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

/// This function serves three purposes:
/// 1. If the result is `Some`, then we will fork the pipeline executing into the background.
/// 2. The value stored within `Some` will be that background job's command name.
/// 3. If `set -x` was set, print the command.
fn check_if_background_job(pipeline: &Pipeline, print_comm: bool) -> Option<String> {
    if pipeline.jobs[pipeline.jobs.len()-1].kind == JobKind::Background {
        let command = pipeline.to_string();
        if print_comm { eprintln!("> {}", command); }
        Some(command)
    } else if print_comm {
        eprintln!("> {}", pipeline.to_string());
        None
    } else {
        None
    }
}

pub trait PipelineExecution {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32;
}

impl<'a> PipelineExecution for Shell<'a> {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32 {
        let background_string = check_if_background_job(&pipeline, self.flags & PRINT_COMMS != 0);

        // Generate a list of commands from the given pipeline
        let mut piped_commands: Vec<(Command, JobKind)> = pipeline.jobs
            .drain(..).map(|mut job| {
                (job.build_command(), job.kind)
            }).collect();
        match pipeline.stdin {
            None => (),
            Some(Input::File(ref filename)) => {
                if let Some(command) = piped_commands.first_mut() {
                    match File::open(filename) {
                        Ok(file) => unsafe {
                            command.0.stdin(Stdio::from_raw_fd(file.into_raw_fd()));
                        },
                        Err(e) => {
                            eprintln!("ion: failed to redirect '{}' into stdin: {}", filename, e);
                        }
                    }
                }
            },
            Some(Input::HereString(ref mut string)) => {
                if let Some(command) = piped_commands.first_mut() {
                    if !string.ends_with('\n') { string.push('\n'); }
                    match unsafe { crossplat::stdin_of(&string) } {
                        Ok(stdio) => {
                            command.0.stdin(stdio);
                        },
                        Err(e) => {
                            eprintln!("ion: failed to redirect herestring '{}' into stdin: {}",
                                      string, e);
                        }
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
        if let Some(command_name) = background_string {
            fork_pipe(self, piped_commands, command_name)
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
                    previous_status = wait(shell, &mut children, remember, foreground);
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
            shell.watch_foreground(child.id(), || get_full_command(command))
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
fn wait (
    shell: &mut Shell,
    children: &mut Vec<Option<Child>>,
    mut commands: Vec<Command>,
    foreground: bool
) -> i32 {
    let end = children.len() - 1;
    for entry in children.drain(..end).zip(commands.drain(..end)) {
        // It is important that `cmd` gets dropped at the end of this
        // block in order to write EOF to the pipes that it owns.
        if let (Some(child), cmd) = entry {
            if foreground { set_foreground(child.id()); }
            let status = shell.watch_foreground(child.id(), || get_full_command(&cmd));
            if status == TERMINATED {
                return status
            }
        }
    }

    if let Some(child) = children.pop().unwrap() {
        let cmd = commands.pop().unwrap();
        if foreground { set_foreground(child.id()); }
        shell.watch_foreground(child.id(), || get_full_command(&cmd))
    } else {
        NO_SUCH_COMMAND
    }
}

fn get_command_name(command: &Command) -> String {
    format!("{:?}", command).split('"').nth(1).unwrap_or("").to_string()
}

fn get_full_command(command: &Command) -> String {
    let command = format!("{:?}", command);
    let mut arg_iter = command.split_whitespace();
    let command = arg_iter.next().unwrap();
    let mut output = String::from(&command[1..command.len()-1]);
    for argument in arg_iter {
        output.push(' ');
        output.push_str(&argument[1..argument.len()-1]);
    }
    output
}
