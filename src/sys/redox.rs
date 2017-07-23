extern crate syscall;

use std::{io, mem, slice};
use std::os::unix::io::RawFd;

use syscall::SigAction;

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
    cvt(syscall::kill(!(pgid as usize), signal as usize)).and(Ok(()))
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
        sa_flags: 0
    };
    cvt(syscall::sigaction(signal as usize, Some(&new), None)).and(Ok(()))
}

pub fn tcsetpgrp(tty_fd: RawFd, pgid: u32) -> io::Result<()> {
    let fd = cvt(syscall::dup(tty_fd, b"pgrp"))?;

    let pgid_usize = pgid as usize;
    let res = syscall::write(fd, unsafe {
        slice::from_raw_parts(
            &pgid_usize as *const usize as *const u8,
            mem::size_of::<usize>()
        )
    });

    let _ = syscall::close(fd);

    cvt(res).and(Ok(()))
}

// Support function for converting syscall error to io error
fn cvt(result: Result<usize, syscall::Error>) -> io::Result<usize> {
    result.map_err(|err| io::Error::from_raw_os_error(err.errno))
}
