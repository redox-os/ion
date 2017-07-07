#[cfg(all(unix, not(target_os = "redox")))] use libc;
#[cfg(all(unix, not(target_os = "redox")))] use nix::unistd::{self, ForkResult};
#[cfg(all(unix, not(target_os = "redox")))] use nix::Error as NixError;
#[cfg(target_os = "redox")] use syscall;
use std::io::{self, Write};
use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, AsRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::fs::{File, OpenOptions};
use std::process::exit;
use super::job_control::{JobControl, ProcessState};
use super::{JobKind, Shell};
use super::status::*;
use parser::peg::{Pipeline, RedirectFrom};

/// The purpose of the signal handler is to ignore signals when it is active, and then continue
/// listening to signals once the handler is dropped.
struct SignalHandler;

impl SignalHandler {
    #[cfg(all(unix, not(target_os = "redox")))]
    pub fn new() -> SignalHandler {
        unsafe { let _ = libc::signal(libc::SIGTTOU, libc::SIG_IGN); }
        SignalHandler
    }

    #[cfg(target_os = "redox")]
    pub fn new() -> SignalHandler {
        // TODO
        SignalHandler
    }
}

impl Drop for SignalHandler {
    #[cfg(all(unix, not(target_os = "redox")))]
    fn drop(&mut self) {
        unsafe { let _ = libc::signal(libc::SIGTTOU, libc::SIG_DFL); }
    }

    #[cfg(target_os = "redox")]
    fn drop(&mut self) {
        // TODO
    }
}

#[cfg(all(unix, not(target_os = "redox")))]
fn unmask_sigtstp() {
    unsafe {
        use libc::{sigset_t, SIG_UNBLOCK, SIGTSTP, sigemptyset, sigaddset, sigprocmask};
        use std::mem;
        use std::ptr;
        let mut sigset = mem::uninitialized::<sigset_t>();
        sigemptyset(&mut sigset as *mut sigset_t);
        sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
        sigprocmask(SIG_UNBLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
    }
}

#[cfg(target_os = "redox")]
fn unmask_sigtstp() {
    // TODO
}

#[cfg(all(unix, not(target_os = "redox")))]
/// When given a process ID, that process will be assigned to a new process group.
fn create_process_group() {
    let _ = unistd::setpgid(0, 0);
}

#[cfg(target_os = "redox")]
fn create_process_group() {
    // TODO
}

#[cfg(all(unix, not(target_os = "redox")))]
/// When given a process ID, that process's group will be assigned as the foreground process group.
pub fn set_foreground(pid: u32) {
    let _ = unistd::tcsetpgrp(0, pid as i32);
    let _ = unistd::tcsetpgrp(1, pid as i32);
    let _ = unistd::tcsetpgrp(2, pid as i32);
}

#[cfg(target_os = "redox")]
pub fn set_foreground(pid: u32) {
    // TODO
}

#[cfg(all(unix, not(target_os = "redox")))]
fn get_pid() -> u32 {
    unistd::getpid() as u32
}

#[cfg(target_os = "redox")]
fn get_pid() -> u32 {
    // TODO
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
            fork_pipe(self, &mut piped_commands)
        } else {
            // While active, the SIGTTOU signal will be ignored.
            let sig_ignore = SignalHandler::new();
            // Execute each command in the pipeline, giving each command the foreground.
            let exit_status = pipe(self, &mut piped_commands, true);
            // Set the shell as the foreground process again to regain the TTY.
            set_foreground(get_pid());
            // Dropping this will un-ignore the SIGTTOU signal.
            drop(sig_ignore);
            exit_status
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
    match unistd::fork()? {
        ForkResult::Parent{ child: pid } => Ok(Fork::Parent(pid as u32)),
        ForkResult::Child                => Ok(Fork::Child)
    }
}

fn fork_pipe(shell: &mut Shell, commands: &mut [(Command, JobKind)]) -> i32 {
    match ion_fork() {
        Ok(Fork::Parent(pid)) => {
            shell.send_to_background(pid, ProcessState::Running);
            SUCCESS
        },
        Ok(Fork::Child) => {
            unmask_sigtstp();
            create_process_group();
            exit(pipe(shell, commands, false));
        },
        Err(why) => {
            eprintln!("ion: background fork failed: {}", why);
            FAILURE
        }
    }
}

/// This function will panic if called with an empty slice
fn pipe(shell: &mut Shell, commands: &mut [(Command, JobKind)], foreground: bool) -> i32 {
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

                let child = command.before_exec(move || {
                    unmask_sigtstp();
                    create_process_group();
                    Ok(())
                }).spawn().ok();
                match child {
                    Some(child) => {
                        if foreground { set_foreground(child.id()); }
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
                    let child = command.before_exec(move || {
                        unmask_sigtstp();
                        create_process_group();
                        Ok(())
                    }).spawn().ok();
                    match child {
                        Some(child) => {
                            if foreground { set_foreground(child.id()); }
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
                previous_status = execute_command(shell, command, foreground);
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

fn execute_command(shell: &mut Shell, command: &mut Command, foreground: bool) -> i32 {
    match command.before_exec(move || {
        unmask_sigtstp();
        create_process_group();
        Ok(())
    }).spawn() {
        Ok(child) => wait_on_child(shell, child, foreground),
        Err(_) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: command not found: {}", get_command_name(command));
            FAILURE
        }
    }
}

fn wait_on_child(shell: &mut Shell, child: Child, foreground: bool) -> i32 {
    if foreground { set_foreground(child.id()); }
    shell.watch_foreground(child.id())
}

/// This function will panic if called with an empty vector
fn wait(shell: &mut Shell, children: &mut Vec<Option<Child>>) -> i32 {
    let end = children.len() - 1;
    for child in children.drain(..end) {
        if let Some(child) = child {
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
