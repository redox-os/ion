#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(target_os = "redox")] use syscall;
use std::io::{self, Write};
use std::process::{Stdio, Command};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::fs::{File, OpenOptions};
use super::flags::*;
use super::fork::{fork_pipe, ion_fork, Fork, create_process_group};
use super::job_control::JobControl;
use super::{JobKind, Shell};
use super::job::RefinedJob;
use super::status::*;
use super::signals::{self, SignalHandler};
use parser::peg::{Pipeline, Input, RedirectFrom};
use sys;
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

    /// Create a File from a byte slice that will echo the contents of the slice
    /// when read. This can be called with owned or borrowed strings
    pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<File, Error> {
        let (reader, writer) = unistd::pipe2(fcntl::O_CLOEXEC)?;
        let mut infile = File::from_raw_fd(writer);
        // Write the contents; make sure to use write_all so that we block until
        // the entire string is written
        infile.write_all(input.as_ref())?;
        infile.flush()?;
        // `infile` currently owns the writer end RawFd. If we just return the reader end
        // and let `infile` go out of scope, it will be closed, sending EOF to the reader!
        Ok(File::from_raw_fd(reader))
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

    pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<File, Error> {
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
        Ok(File::from_raw_fd(reader))
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

        let mut piped_commands: Vec<(RefinedJob, JobKind)> = {
            pipeline.jobs
                .drain(..)
                .map(|job| {
                    let refined = if self.builtins.contains_key(job.command.as_ref()) {
                        RefinedJob::builtin(
                            job.command,
                            job.args.drain().skip(1).collect()
                        )
                    } else {
                        let command = Command::new(job.command);
                        for arg in job.args.drain().skip(1) {
                            command.arg(arg);
                        }
                        RefinedJob::External(command)
                    };
                    (refined, job.kind)
                })
                .collect()
        };
        match pipeline.stdin {
            None => (),
            Some(Input::File(ref filename)) => {
                if let Some(command) = piped_commands.first_mut() {
                    match File::open(filename) {
                        Ok(file) => command.0.stdin(file),
                        Err(e) => {
                            eprintln!("ion: failed to redirect '{}' into stdin: {}",
                                      filename, e)
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
                            eprintln!(
                                "ion: failed to redirect herestring '{}' into stdin: {}",
                                string,
                                e
                            );
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
                                match f.try_clone() {
                                    Ok(f_copy) => {
                                        command.0.stdout(f);
                                        command.0.stderr(f_copy);
                                    },
                                    Err(e) => {
                                        eprintln!("ion: failed to redirect both stderr and stdout into file '{:?}'", f);
                                    }
                                }
                            },
                            RedirectFrom::Stderr => command.0.stderr(f),
                            RedirectFrom::Stdout => command.0.stdout(f),
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
    commands: Vec<(RefinedJob, JobKind)>,
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
                    continue
                },
                JobKind::Or => if previous_status == SUCCESS {
                    if let JobKind::And = kind { previous_kind = kind }
                    continue
                },
                _ => ()
            }

            match kind {
                JobKind::Pipe(mut mode) => {
                    // We need to remember the commands as they own the file
                    // descriptors that are created by sys::pipe.
                    // We purposfully drop the pipes that are owned by a given
                    // command in `wait` in order to close those pipes, sending
                    // EOF to the next command
                    let mut remember = Vec::new();
                    // A list of the PIDs in the piped command
                    let mut children: Vec<u32> = Vec::new();
                    // The process group by which all of the PIDs belong to.
                    let mut pgid = 0; // 0 means the PGID is not set yet.

                    macro_rules! spawn_proc {
                        ($cmd:expr) => {{
                            match $cmd {
                                &mut RefinedJob::External(ref mut command) => {
                                    match {
                                        command.before_exec(move || {
                                            signals::unblock();
                                            create_process_group(pgid);
                                            Ok(())
                                        }).spawn()
                                    } {
                                        Ok(child) => {
                                            if pgid == 0 {
                                                pgid = child.id();
                                                if foreground {
                                                    set_foreground(pgid);
                                                }
                                            }
                                            shell.foreground.push(child.id());
                                            children.push(child.id());
                                        },
                                        Err(e) => {
                                            eprintln!("ion: failed to spawn `{}`: {}",
                                                      $cmd.short(), e);
                                            return NO_SUCH_COMMAND
                                        }
                                    }
                                }
                                &mut RefinedJob::Builtin { name,
                                                           args,
                                                           stdin,
                                                           stdout,
                                                           stderr } =>
                                {
                                    match ion_fork() {
                                        Ok(Fork::Parent(pid)) => {
                                            if pgid == 0 {
                                                pgid = pid;
                                                if foreground {
                                                    set_foreground(pgid);
                                                }
                                            }
                                            shell.foreground.push(pid);
                                            children.push(pid);
                                        },
                                        Ok(Fork::Child) => {
                                            signals::unblock();
                                            create_process_group(pgid);
                                            unimplemented!()
                                        }
                                    }
                                }
                            }
                        }};
                    }

                    // Append other jobs until all piped jobs are running
                    while let Some((mut child, ckind)) = commands.next() {
                        match sys::pipe2(sys::O_CLOEXEC) {
                            Err(e) =>  {
                                eprintln!("ion: failed to create pipe: {:?}", e);
                            },
                            Ok((reader, writer)) => {
                                reader.foo();
                                writer.foo();
                                child.stdin(reader);
                                match mode {
                                    RedirectFrom::Stderr => {
                                        parent.stderr(writer);
                                    },
                                    RedirectFrom::Stdout => {
                                        parent.stdout(writer);
                                    },
                                    RedirectFrom::Both => {
                                        let temp = File::from_raw_fd(writer);
                                        match temp.try_clone() {
                                            Err(e) => {
                                                eprintln!("ion: failed to redirect stdout and stderr");
                                            }
                                            Ok(duped) => {
                                                parent.stderr(temp);
                                                parent.stdout(duped);
                                            }
                                        }
                                    }
                                }
                            }
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
                    unimplemented!();
                    previous_kind = kind;
                    previous_status = wait(shell, children, remember);
                    if previous_status == TERMINATED {
                        terminate_fg(shell);
                        return previous_status;
                    }
                }
                _ => {
                    previous_status = execute(shell, &mut parent, foreground);
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

fn execute(shell: &mut Shell, job: &mut RefinedJob, foreground: bool) -> i32 {
    match job {
        &mut RefinedJob::External(ref mut command) => {
            match {
                command.before_exec(move || {
                    signals::unblock();
                    create_process_group(0);
                    Ok(())
                }).spawn()
            } {
                Ok(child) => {
                    if foreground {
                        set_foreground(child.id());
                    }
                    shell.watch_foreground(
                        child.id(),
                        child.id(),
                        || job.long(),
                        |_| ())
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound {
                        eprintln!("ion: command not found: {}", job.short())
                    } else {
                        eprintln!("ion: error spawning process: {}", e)
                    };
                    FAILURE
                }
            }
        }
        &mut RefinedJob::Builtin { name,
                                   args,
                                   stdin,
                                   stdout,
                                   stderr } =>
        {
            match ion_fork() {
                Ok(Fork::Parent(pid)) => {
                    if foreground {
                       set_foreground(pid);
                    }
                    shell.watch_foreground(pid, pid, || job.long(), |_| ())
                },
                Ok(Fork::Child) => {
                    signals::unblock();
                    create_process_group(0);
                    unimplemented!()
                },
                Err(e) => {
                    eprintln!("ion: fork error: {}", e);
                    FAILURE
                }
            }
        }
    }
}

/// Waits for all of the children within a pipe to finish exuecting, returning the
/// exit status of the last process in the queue.
fn wait (
    shell: &mut Shell,
    mut children: Vec<u32>,
    mut commands: Vec<RefinedJob>
) -> i32 {
    // TODO: Find a way to only do this when absolutely necessary.
    let as_string = commands.iter()
        .map(RefinedJob::long)
        .collect::<Vec<String>>()
        .join(" | ");

    // Each process in the pipe has the same PGID, which is the first process's PID.
    let pgid = children[0];
    // If the last process exits, we know that all processes should exit.
    let last_pid = children[children.len()-1];

    // Watch the foreground group, dropping all commands that exit as they exit.
    shell.watch_foreground(pgid, last_pid, move || as_string, move |pid| {
        if let Some(id) = children.iter().position(|&x| x as i32 == pid) {
            commands.remove(id);
            children.remove(id);
        }
    })
}
