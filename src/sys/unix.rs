extern crate libc;

use libc::{
    c_int,
    pid_t,
    sighandler_t
};
use std::io;
use std::os::unix::io::RawFd;

pub const O_CLOEXEC: usize = libc::O_CLOEXEC as usize;
pub const SIGHUP: i32 = libc::SIGHUP;
pub const SIGINT: i32 = libc::SIGINT;
pub const SIGTERM: i32 = libc::SIGTERM;
pub const SIGCONT: i32 = libc::SIGCONT;
pub const SIGSTOP: i32 = libc::SIGSTOP;
pub const SIGTSTP: i32 = libc::SIGTSTP;
pub const SIGTTOU: i32 = libc::SIGTTOU;

pub unsafe fn fork() -> io::Result<u32> {
    cvt(libc::fork()).map(|pid| pid as u32)
}

pub fn getpid() -> io::Result<u32> {
    cvt(unsafe { libc::getpid() }).map(|pid| pid as u32)
}

pub fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(pid as pid_t, signal as c_int) }).and(Ok(()))
}

pub fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(-(pgid as pid_t), signal as c_int) }).and(Ok(()))
}

pub fn pipe2(flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];
    cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), flags as c_int) })?;
    Ok((fds[0], fds[1]))
}

pub fn setpgid(pid: u32, pgid: u32) -> io::Result<()> {
    cvt(unsafe { libc::setpgid(pid as pid_t, pgid as pid_t) }).and(Ok(()))
}

pub fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, handler as sighandler_t) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn ignore(signal: i32) -> io::Result<()> {
    if unsafe { libc::signal(signal as libc::c_int, libc::SIG_IGN) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn tcsetpgrp(fd: RawFd, pgrp: u32) -> io::Result<()> {
    cvt(unsafe { libc::tcsetpgrp(fd as c_int, pgrp as pid_t) }).and(Ok(()))
}

// Support functions for converting libc return values to io errors {
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
// } End of support functions
