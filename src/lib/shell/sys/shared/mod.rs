use libc::{c_int, pid_t, sighandler_t};
use std::{ffi::CStr, io, os::unix::io::RawFd};
pub mod signals;

pub const O_CLOEXEC: usize = libc::O_CLOEXEC as usize;
pub const SIGHUP: i32 = libc::SIGHUP;
pub const SIGINT: i32 = libc::SIGINT;
pub const SIGTERM: i32 = libc::SIGTERM;
pub const SIGCONT: i32 = libc::SIGCONT;
pub const SIGSTOP: i32 = libc::SIGSTOP;
pub const SIGTSTP: i32 = libc::SIGTSTP;
pub const SIGPIPE: i32 = libc::SIGPIPE;

pub const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
pub const STDERR_FILENO: i32 = libc::STDERR_FILENO;
pub const STDIN_FILENO: i32 = libc::STDIN_FILENO;

pub use libc::{ECHILD, EINTR, WCONTINUED, WNOHANG, WUNTRACED};

// Why each platform wants to be unique in this regard is anyone's guess.
#[cfg(target_os = "linux")]
fn errno() -> i32 { unsafe { *libc::__errno_location() } }

#[cfg(any(target_os = "openbsd", target_os = "bitrig", target_os = "android"))]
fn errno() -> i32 { unsafe { *libc::__errno() } }

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
fn errno() -> i32 { unsafe { *libc::__error() } }

#[cfg(target_os = "dragonfly")]
fn errno() -> i32 { unsafe { *errno_dragonfly::errno_location() } }

pub fn strerror(errno: i32) -> &'static str {
    unsafe {
        let ptr = libc::strerror(errno);
        if ptr.is_null() {
            return "Unknown Error";
        }

        CStr::from_ptr(ptr).to_str().unwrap_or("Unknown Error")
    }
}

pub fn waitpid(pid: i32, status: &mut i32, options: i32) -> Result<i32, i32> {
    match unsafe { libc::waitpid(pid, status, options) } {
        -1 => Err(errno()),
        pid => Ok(pid),
    }
}

pub fn wexitstatus(status: i32) -> i32 { unsafe { libc::WEXITSTATUS(status) } }
pub fn wifexited(status: i32) -> bool { unsafe { libc::WIFEXITED(status) } }
pub fn wifstopped(status: i32) -> bool { unsafe { libc::WIFSTOPPED(status) } }
pub fn wifcontinued(status: i32) -> bool { unsafe { libc::WIFCONTINUED(status) } }
pub fn wifsignaled(status: i32) -> bool { unsafe { libc::WIFSIGNALED(status) } }
pub fn wcoredump(status: i32) -> bool { unsafe { libc::WCOREDUMP(status) } }
pub fn wtermsig(status: i32) -> i32 { unsafe { libc::WTERMSIG(status) } }
pub fn wstopsig(status: i32) -> i32 { unsafe { libc::WSTOPSIG(status) } }

pub fn getpid() -> io::Result<u32> { cvt(unsafe { libc::getpid() }).map(|pid| pid as u32) }
pub fn geteuid() -> io::Result<u32> { Ok(unsafe { libc::geteuid() } as u32) }
pub fn getuid() -> io::Result<u32> { Ok(unsafe { libc::getuid() } as u32) }

pub unsafe fn fork() -> io::Result<u32> { cvt(libc::fork()).map(|pid| pid as u32) }
pub fn fork_exit(exit_status: i32) -> ! { unsafe { libc::_exit(exit_status) } }

pub fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(pid as pid_t, signal as c_int) }).and(Ok(()))
}
pub fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(-(pgid as pid_t), signal as c_int) }).and(Ok(()))
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

pub fn reset_signal(signal: i32) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, libc::SIG_DFL) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn tcsetpgrp(fd: RawFd, pgrp: u32) -> io::Result<()> {
    cvt(unsafe { libc::tcsetpgrp(fd as c_int, pgrp as pid_t) }).and(Ok(()))
}

pub fn dup(fd: RawFd) -> io::Result<RawFd> { cvt(unsafe { libc::dup(fd) }) }
pub fn dup2(old: RawFd, new: RawFd) -> io::Result<RawFd> { cvt(unsafe { libc::dup2(old, new) }) }
pub fn close(fd: RawFd) -> io::Result<()> { cvt(unsafe { libc::close(fd) }).and(Ok(())) }
pub fn isatty(fd: RawFd) -> bool { unsafe { libc::isatty(fd) == 1 } }

pub fn pipe2(flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];

    #[cfg(not(target_os = "macos"))]
    cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), flags as c_int) })?;

    #[cfg(target_os = "macos")]
    cvt(unsafe { libc::pipe(fds.as_mut_ptr()) })?;

    Ok((fds[0], fds[1]))
}

pub mod variables {
    use libc::c_char;
    use users::{get_user_by_name, os::unix::UserExt};

    pub fn get_user_home(username: &str) -> Option<String> {
        match get_user_by_name(username) {
            Some(user) => Some(user.home_dir().to_string_lossy().into_owned()),
            None => None,
        }
    }

    pub fn get_host_name() -> Option<String> {
        let mut host_name = [0u8; 512];

        if unsafe { libc::gethostname(&mut host_name as *mut _ as *mut c_char, host_name.len()) }
            == 0
        {
            let len = host_name.iter().position(|i| *i == 0).unwrap_or_else(|| host_name.len());

            Some(unsafe { String::from_utf8_unchecked(host_name[..len].to_owned()) })
        } else {
            None
        }
    }
}

pub mod env {
    use libc;
    use std::{
        env,
        ffi::{CStr, OsString},
        mem,
        os::unix::ffi::OsStringExt,
        path::PathBuf,
        ptr,
    };

    pub fn home_dir() -> Option<PathBuf> {
        return env::var_os("HOME").or_else(|| unsafe { fallback() }).map(PathBuf::from);

        #[cfg(any(
            target_os = "android",
            target_os = "ios",
            target_os = "emscripten",
            target_os = "redox"
        ))]
        unsafe fn fallback() -> Option<OsString> { None }
        #[cfg(not(any(
            target_os = "android",
            target_os = "ios",
            target_os = "emscripten",
            target_os = "redox"
        )))]
        unsafe fn fallback() -> Option<OsString> {
            let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
                n if n < 0 => 512 as usize,
                n => n as usize,
            };
            let mut buf = Vec::with_capacity(amt);
            let mut passwd: libc::passwd = mem::zeroed();
            let mut result = ptr::null_mut();
            match libc::getpwuid_r(
                libc::getuid(),
                &mut passwd,
                buf.as_mut_ptr(),
                buf.capacity(),
                &mut result,
            ) {
                0 if !result.is_null() => {
                    let ptr = passwd.pw_dir as *const _;
                    let bytes = CStr::from_ptr(ptr).to_bytes().to_vec();
                    Some(OsStringExt::from_vec(bytes))
                }
                _ => None,
            }
        }
    }
}

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
