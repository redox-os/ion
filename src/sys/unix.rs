extern crate libc;

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

pub mod signals {
    /// Blocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so that the shell never receives
    /// them.
    pub(crate) fn block() // fn block() // fn block() // fn block() // fn block()
    {
        unsafe {
            use libc::*;
            use std::mem;
            use std::ptr;
            let mut sigset = mem::uninitialized::<sigset_t>();
            sigemptyset(&mut sigset as *mut sigset_t);
            sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
            sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
            sigprocmask(SIG_BLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
        }
    }

    /// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be
    /// controlled
    /// by the shell.
    pub(crate) fn unblock() // fn unblock() // fn unblock() // fn unblock() // fn unblock()
    {
        unsafe {
            use libc::*;
            use std::mem;
            use std::ptr;
            let mut sigset = mem::uninitialized::<sigset_t>();
            sigemptyset(&mut sigset as *mut sigset_t);
            sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
            sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
            sigprocmask(SIG_UNBLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
        }
    }
}

pub mod job_control {
    use shell::job_control::*;

    use libc::{self, pid_t};
    use shell::Shell;
    use shell::foreground::ForegroundSignals;
    use shell::status::{FAILURE, TERMINATED};
    use std::sync::{Arc, Mutex};
    use std::thread::sleep;
    use std::time::Duration;

    use nix::sys::wait::{waitpid, WaitStatus, WNOHANG, WUNTRACED};
    #[cfg(not(target_os = "macos"))]
    use nix::sys::wait::WCONTINUED;

    use nix::{Errno, Error};
    use nix::sys::signal::Signal;

    pub(crate) fn watch_background(
        fg: Arc<ForegroundSignals>,
        processes: Arc<Mutex<Vec<BackgroundProcess>>>,
        pid: u32,
        njob: usize,
    ) {
        let mut fg_was_grabbed = false;
        loop {
            if !fg_was_grabbed {
                if fg.was_grabbed(pid) {
                    fg_was_grabbed = true;
                }
            }

            #[cfg(not(target_os = "macos"))]
            let opts = Some(WUNTRACED | WCONTINUED | WNOHANG);
            #[cfg(target_os = "macos")]
            let opts = Some(WUNTRACED | WNOHANG);

            match waitpid(-(pid as pid_t), opts) {
                Ok(WaitStatus::Exited(_, status)) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) exited with {}", njob, pid, status);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.reply_with(status);
                    }
                    break;
                }
                Ok(WaitStatus::Stopped(pid, _)) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Stopped", njob, pid);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    if fg_was_grabbed {
                        fg.reply_with(TERMINATED as i8);
                        fg_was_grabbed = false;
                    }
                    process.state = ProcessState::Stopped;
                }
                Ok(WaitStatus::Continued(pid)) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Running", njob, pid);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Running;
                }
                Ok(_) => (),
                Err(why) => {
                    eprintln!("ion: ([{}] {}) errored: {}", njob, pid, why);
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.errored();
                    }
                    break;
                }
            }
            sleep(Duration::from_millis(100));
        }
    }

    pub(crate) fn watch_foreground<'a, F, D>(
        shell: &mut Shell<'a>,
        _pid: u32,
        last_pid: u32,
        get_command: F,
        mut drop_command: D,
    ) -> i32
        where F: FnOnce() -> String,
              D: FnMut(i32)
    {
        let mut exit_status = 0;
        loop {
            match waitpid(-1, Some(WUNTRACED)) {
                Ok(WaitStatus::Exited(pid, status)) => if pid == (last_pid as i32) {
                    break status as i32;
                } else {
                    drop_command(pid);
                    exit_status = status;
                },
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    eprintln!("ion: process ended by signal");
                    if signal == Signal::SIGTERM {
                        shell.handle_signal(libc::SIGTERM);
                        shell.exit(TERMINATED);
                    } else if signal == Signal::SIGHUP {
                        shell.handle_signal(libc::SIGHUP);
                        shell.exit(TERMINATED);
                    } else if signal == Signal::SIGINT {
                        shell.foreground_send(libc::SIGINT as i32);
                        shell.break_flow = true;
                    }
                    break TERMINATED;
                }
                Ok(WaitStatus::Stopped(pid, _)) => {
                    shell.send_to_background(pid as u32, ProcessState::Stopped, get_command());
                    shell.break_flow = true;
                    break TERMINATED;
                }
                Ok(_) => (),
                // ECHILD signifies that all children have exited
                Err(Error::Sys(Errno::ECHILD)) => break exit_status as i32,
                Err(why) => {
                    eprintln!("ion: process doesn't exist: {}", why);
                    break FAILURE;
                }
            }
        }
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
