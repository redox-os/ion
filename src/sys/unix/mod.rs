extern crate libc;

pub mod job_control;
pub mod signals;

use libc::{c_int, pid_t, sighandler_t};
use std::io;
use std::os::unix::io::RawFd;

pub(crate) const PATH_SEPARATOR: &str = ":";

pub(crate) const O_CLOEXEC: usize = libc::O_CLOEXEC as usize;
pub(crate) const SIGHUP: i32 = libc::SIGHUP;
pub(crate) const SIGINT: i32 = libc::SIGINT;
pub(crate) const SIGTERM: i32 = libc::SIGTERM;
pub(crate) const SIGCONT: i32 = libc::SIGCONT;
pub(crate) const SIGSTOP: i32 = libc::SIGSTOP;
pub(crate) const SIGTSTP: i32 = libc::SIGTSTP;

pub(crate) const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
pub(crate) const STDERR_FILENO: i32 = libc::STDERR_FILENO;
pub(crate) const STDIN_FILENO: i32 = libc::STDIN_FILENO;

pub(crate) fn is_root() -> bool { unsafe { libc::geteuid() == 0 } }

pub unsafe fn fork() -> io::Result<u32> { cvt(libc::fork()).map(|pid| pid as u32) }

pub(crate) fn getpid() -> io::Result<u32> { cvt(unsafe { libc::getpid() }).map(|pid| pid as u32) }

pub(crate) fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(pid as pid_t, signal as c_int) }).and(Ok(()))
}

pub(crate) fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(-(pgid as pid_t), signal as c_int) }).and(Ok(()))
}

pub(crate) fn pipe2(flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];

    #[cfg(not(target_os = "macos"))]
    cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), flags as c_int) })?;

    #[cfg(target_os = "macos")]
    cvt(unsafe { libc::pipe(fds.as_mut_ptr()) })?;

    Ok((fds[0], fds[1]))
}

pub(crate) fn setpgid(pid: u32, pgid: u32) -> io::Result<()> {
    cvt(unsafe { libc::setpgid(pid as pid_t, pgid as pid_t) }).and(Ok(()))
}

#[allow(dead_code)]
pub(crate) fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, handler as sighandler_t) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn reset_signal(signal: i32) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, libc::SIG_DFL) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn tcsetpgrp(fd: RawFd, pgrp: u32) -> io::Result<()> {
    cvt(unsafe { libc::tcsetpgrp(fd as c_int, pgrp as pid_t) }).and(Ok(()))
}

pub(crate) fn dup(fd: RawFd) -> io::Result<RawFd> { cvt(unsafe { libc::dup(fd) }) }

pub(crate) fn dup2(old: RawFd, new: RawFd) -> io::Result<RawFd> {
    cvt(unsafe { libc::dup2(old, new) })
}

pub(crate) fn close(fd: RawFd) -> io::Result<()> { cvt(unsafe { libc::close(fd) }).and(Ok(())) }

pub(crate) fn isatty(fd: RawFd) -> bool { unsafe { libc::isatty(fd) == 1 } }

trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
        ($($t:ident)*) => ($(impl IsMinusOne for $t {
            fn is_minus_one(&self) -> bool {
                *self == -1
            }
        })*)
    }

impl_is_minus_one! { i8 i16 i32 i64 isize }

fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}

pub mod variables {
    use users_unix::get_user_by_name;
    use users_unix::os::unix::UserExt;

    pub(crate) fn get_user_home(username: &str) -> Option<String> {
        match get_user_by_name(username) {
            Some(user) => Some(user.home_dir().to_string_lossy().into_owned()),
            None => None,
        }
    }
}
