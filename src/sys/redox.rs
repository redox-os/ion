extern crate syscall;

use std::{io, mem, slice};
use std::os::unix::io::RawFd;

use syscall::SigAction;

pub const PATH_SEPARATOR: &str = ";";

pub const O_CLOEXEC: usize = syscall::O_CLOEXEC;
pub const SIGHUP: i32 = syscall::SIGHUP as i32;
pub const SIGINT: i32 = syscall::SIGINT as i32;
pub const SIGTERM: i32 = syscall::SIGTERM as i32;
pub const SIGCONT: i32 = syscall::SIGCONT as i32;
pub const SIGSTOP: i32 = syscall::SIGSTOP as i32;
pub const SIGTSTP: i32 = syscall::SIGTSTP as i32;

pub unsafe fn fork() -> io::Result<u32> {
    cvt(syscall::clone(0)).map(|pid| pid as u32)
}

pub fn getpid() -> io::Result<u32> {
    cvt(syscall::getpid()).map(|pid| pid as u32)
}

pub fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(syscall::kill(pid as usize, signal as usize)).and(Ok(()))
}

pub fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(syscall::kill(-(pgid as isize) as usize, signal as usize)).and(Ok(()))
}

pub fn pipe2(flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];
    cvt(syscall::pipe2(&mut fds, flags))?;
    Ok((fds[0], fds[1]))
}

pub fn setpgid(pid: u32, pgid: u32) -> io::Result<()> {
    cvt(syscall::setpgid(pid as usize, pgid as usize)).and(Ok(()))
}

pub fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    let new = SigAction {
        sa_handler: unsafe { mem::transmute(handler) },
        sa_mask: [0; 2],
        sa_flags: 0,
    };
    cvt(syscall::sigaction(signal as usize, Some(&new), None)).and(Ok(()))
}

pub fn tcsetpgrp(tty_fd: RawFd, pgid: u32) -> io::Result<()> {
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

// Support function for converting syscall error to io error
fn cvt(result: Result<usize, syscall::Error>) -> io::Result<usize> {
    result.map_err(|err| io::Error::from_raw_os_error(err.errno))
}

// TODO
pub mod signals {
    pub fn block() {}

    /// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be controlled
    /// by the shell.
    pub fn unblock() {}
}

pub mod job_control {
    use shell::job_control::*;

    use std::io::{self, Write};
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::{Arc, Mutex};
    use syscall;
    use shell::foreground::ForegroundSignals;
    use shell::status::TERMINATED;
    use shell::Shell;

    pub fn watch_background(
        fg: Arc<ForegroundSignals>,
        processes: Arc<Mutex<Vec<BackgroundProcess>>>,
        pid: u32,
        njob: usize,
    ) {
        // TODO: Implement this using syscall::call::waitpid
    }


    pub fn watch_foreground<'a, F, D>(
        _shell: &mut Shell<'a>,
        pid: u32,
        _last_pid: u32,
        _get_command: F,
        mut drop_command: D,
    ) -> i32
    where
        F: FnOnce() -> String,
        D: FnMut(i32),
    {
        loop {
            let mut status_raw = 0;
            match syscall::waitpid(pid as usize, &mut status_raw, 0) {
                Ok(0) => (),
                Ok(_pid) => {
                    let status = ExitStatus::from_raw(status_raw as i32);
                    if let Some(code) = status.code() {
                        break code;
                    } else {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = stderr.write_all(b"ion: child ended by signal\n");
                        break TERMINATED;
                    }
                }
                Err(err) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                    break 100; // TODO what should we return here?
                }
            }
        }
    }
}

pub mod variables {
    pub fn get_user_home(_username: &str) -> Option<String> {
        // TODO
        None
    }
}
