extern crate syscall;

use std::{io, mem, slice};
use std::env;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::PathBuf;
use std::process::{exit, ExitStatus};
use std::os::unix::process::ExitStatusExt;
use syscall::{EINTR, SigAction, waitpid};

pub(crate) const PATH_SEPARATOR: &str = ";";
pub(crate) const NULL_PATH: &str = "null:";

pub(crate) const O_CLOEXEC: usize = syscall::O_CLOEXEC;
pub(crate) const SIGHUP: i32 = syscall::SIGHUP as i32;
pub(crate) const SIGINT: i32 = syscall::SIGINT as i32;
pub(crate) const SIGTERM: i32 = syscall::SIGTERM as i32;
pub(crate) const SIGCONT: i32 = syscall::SIGCONT as i32;
pub(crate) const SIGSTOP: i32 = syscall::SIGSTOP as i32;
pub(crate) const SIGTSTP: i32 = syscall::SIGTSTP as i32;
pub(crate) const SIGPIPE: i32 = syscall::SIGPIPE as i32;

pub(crate) const STDIN_FILENO: RawFd = 0;
pub(crate) const STDOUT_FILENO: RawFd = 1;
pub(crate) const STDERR_FILENO: RawFd = 2;

pub(crate) fn geteuid() -> io::Result<u32> { cvt(syscall::geteuid()).map(|pid| pid as u32) }

pub(crate) fn getuid() -> io::Result<u32> { cvt(syscall::getuid()).map(|pid| pid as u32) }

pub(crate) fn is_root() -> bool { syscall::geteuid().map(|id| id == 0).unwrap_or(false) }

pub unsafe fn fork() -> io::Result<u32> { cvt(syscall::clone(0)).map(|pid| pid as u32) }

pub fn fork_exit(status: i32) -> ! { exit(status) }

pub fn wait_for_interrupt(pid: u32) -> io::Result<()> {
    let mut status = 0;

    loop {
        match waitpid(pid as usize, &mut status, 0) {
            Err(ref error) if error.errno == EINTR => continue,
            Err(ref error) => break Err(io::Error::from_raw_os_error(error.errno)),
            Ok(_) => break Ok(()),
        }
    }
}

pub fn wait_for_child(pid: u32) -> io::Result<u8> {
    let mut status;
    use syscall::{waitpid, ECHILD};

    loop {
        status = 0;
        match waitpid(pid as usize, &mut status, 0) {
            Err(ref error) if error.errno == ECHILD => break,
            Err(error) => return Err(io::Error::from_raw_os_error(error.errno)),
            _ => ()
        }
    }

    let status = ExitStatus::from_raw(status as i32);
    Ok(status.code().unwrap_or(0) as u8)
}

pub(crate) fn getpid() -> io::Result<u32> { cvt(syscall::getpid()).map(|pid| pid as u32) }

pub(crate) fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(syscall::kill(pid as usize, signal as usize)).and(Ok(()))
}

pub(crate) fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(syscall::kill(-(pgid as isize) as usize, signal as usize)).and(Ok(()))
}

pub(crate) fn pipe2(flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];
    cvt(syscall::pipe2(&mut fds, flags))?;
    Ok((fds[0], fds[1]))
}

pub(crate) fn setpgid(pid: u32, pgid: u32) -> io::Result<()> {
    cvt(syscall::setpgid(pid as usize, pgid as usize)).and(Ok(()))
}

pub(crate) fn execve(prog: &str, args: &[&str], clear_env: bool) -> io::Result<()> {
    // Construct a valid set of arguments to pass to execve. Ensure
    // that the program is the first argument.
    let mut cvt_args: Vec<[usize; 2]> = Vec::new();
    cvt_args.push([prog.as_ptr() as usize, prog.len()]);
    for arg in args {
        cvt_args.push([arg.as_ptr() as usize, arg.len()]);
    }

    // Get the PathBuf of the program if it exists.
    let prog = if prog.contains(':') || prog.contains('/') {
        // This is a fully specified scheme or path to an
        // executable.
        Some(PathBuf::from(prog))
    } else if let Ok(paths) = env::var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        env::split_paths(&paths)
            .filter_map(|mut path| {
                path.push(prog);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            })
            .next()
    } else {
        None
    };

    // If clear_env set, clear the env.
    if clear_env {
        for (key, _) in env::vars() {
            env::remove_var(key);
        }
    }

    if let Some(prog) = prog {
        // If we found the program. Run it!
        cvt(syscall::execve(prog.as_os_str().as_bytes(), &cvt_args)).and(Ok(()))
    } else {
        // The binary was not found.
        Err(io::Error::from_raw_os_error(syscall::ENOENT))
    }
}

#[allow(dead_code)]
pub(crate) fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    let new = SigAction {
        sa_handler: unsafe { mem::transmute(handler) },
        sa_mask:    [0; 2],
        sa_flags:   0,
    };
    cvt(syscall::sigaction(signal as usize, Some(&new), None)).and(Ok(()))
}

pub(crate) fn reset_signal(signal: i32) -> io::Result<()> {
    let new = SigAction {
        sa_handler: unsafe { mem::transmute(syscall::flag::SIG_DFL) },
        sa_mask:    [0; 2],
        sa_flags:   0,
    };
    cvt(syscall::sigaction(signal as usize, Some(&new), None)).and(Ok(()))
}

pub(crate) fn tcsetpgrp(tty_fd: RawFd, pgid: u32) -> io::Result<()> {
    let fd = cvt(syscall::dup(tty_fd, b"pgrp"))?;

    let pgid_usize = pgid as usize;
    let res = syscall::write(fd, unsafe {
        slice::from_raw_parts(
            &pgid_usize as *const usize as *const u8,
            mem::size_of::<usize>(),
        )
    });

    let _ = syscall::close(fd);

    cvt(res).and(Ok(()))
}

pub(crate) fn dup(fd: RawFd) -> io::Result<RawFd> { cvt(syscall::dup(fd, &[])) }

pub(crate) fn dup2(old: RawFd, new: RawFd) -> io::Result<RawFd> {
    cvt(syscall::dup2(old, new, &[]))
}

pub(crate) fn close(fd: RawFd) -> io::Result<()> { cvt(syscall::close(fd)).and(Ok(())) }

pub(crate) fn isatty(fd: RawFd) -> bool {
    if let Ok(tfd) = syscall::dup(fd, b"termios") {
        let _ = syscall::close(tfd);
        true
    } else {
        false
    }
}

// Support function for converting syscall error to io error
fn cvt(result: Result<usize, syscall::Error>) -> io::Result<usize> {
    result.map_err(|err| io::Error::from_raw_os_error(err.errno))
}

// TODO
pub mod signals {
    pub(crate) fn block() {}

    /// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be
    /// controlled
    /// by the shell.
    pub(crate) fn unblock() {}
}

pub mod job_control {
    use shell::job_control::*;

    use shell::Shell;
    use shell::foreground::ForegroundSignals;
    use shell::status::FAILURE;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::{Arc, Mutex};
    use syscall::{ECHILD, waitpid};
    use super::{SIGINT, SIGPIPE};

    pub(crate) fn watch_background(
        _fg: Arc<ForegroundSignals>,
        _processes: Arc<Mutex<Vec<BackgroundProcess>>>,
        _pid: u32,
        _njob: usize,
    ) {
        // TODO: Implement this using syscall::call::waitpid
    }

    pub(crate) fn watch_foreground(shell: &mut Shell, pid: i32, command: &str ) -> i32 {
        let mut signaled = 0;
        let mut exit_status = 0;
        let mut status;

        fn get_pid_value(pid: i32) -> usize {
            if pid < 0 {
                !(pid.abs() as usize)
            } else {
                pid as usize
            }
        }

        loop {
            status = 0;
            let result = waitpid(get_pid_value(pid), &mut status, 0);
            match result {
                Err(error) => {
                    match error.errno {
                        ECHILD if signaled == 0 => break exit_status,
                        ECHILD => break signaled,
                        _ => {
                            eprintln!("ion: waitpid error: {}", error);
                            break FAILURE;
                        }
                    }
                }
                Ok(0) => (),
                Ok(pid) => {
                    let es = ExitStatus::from_raw(status as i32);
                    match es.signal() {
                        Some(SIGPIPE) => continue,
                        Some(signal) => {
                            eprintln!("ion: process ended by signal {}", signal);
                            match signal {
                                SIGINT => {
                                    shell.foreground_send(signal as i32);
                                    shell.break_flow = true;
                                }
                                _ => {
                                    shell.handle_signal(signal);
                                }
                            }
                            signaled = 128 + signal as i32;
                        }
                        None => {
                            exit_status = es.code().unwrap();
                        }
                    }
                }
                // TODO: Background job control for Redox
                // _pid if WIFSTOPPED(status) => {
                //     shell.send_to_background(pid as u32, ProcessState::Stopped, command.into());
                //     shell.break_flow = true;
                //     break 128 + signal as i32;
                // }
            }
        }
    }
}

pub mod variables {
    pub(crate) fn get_user_home(_username: &str) -> Option<String> {
        // TODO
        None
    }
}
