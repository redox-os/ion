extern crate libc;

pub mod job_control;
pub mod signals;

use libc::{
    c_char, c_int, pid_t, sighandler_t, strerror, waitpid, ECHILD, EINTR, WEXITSTATUS, WUNTRACED,
};
use std::{
    env, ffi::{CStr, CString}, io::{self, Write}, os::unix::io::RawFd, ptr,
};

pub(crate) const PATH_SEPARATOR: &str = ":";
pub(crate) const NULL_PATH: &str = "/dev/null";

pub(crate) const O_CLOEXEC: usize = libc::O_CLOEXEC as usize;
pub(crate) const SIGHUP: i32 = libc::SIGHUP;
pub(crate) const SIGINT: i32 = libc::SIGINT;
pub(crate) const SIGTERM: i32 = libc::SIGTERM;
pub(crate) const SIGCONT: i32 = libc::SIGCONT;
pub(crate) const SIGSTOP: i32 = libc::SIGSTOP;
pub(crate) const SIGTSTP: i32 = libc::SIGTSTP;
pub(crate) const SIGPIPE: i32 = libc::SIGPIPE;

pub(crate) const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
pub(crate) const STDERR_FILENO: i32 = libc::STDERR_FILENO;
pub(crate) const STDIN_FILENO: i32 = libc::STDIN_FILENO;

// Why each platform wants to be unique in this regard is anyone's guess.

#[cfg(target_os = "linux")]
fn errno() -> i32 { unsafe { *libc::__errno_location() } }

#[cfg(any(target_os = "openbsd", target_os = "bitrig", target_os = "android"))]
fn errno() -> i32 { unsafe { *libc::__errno() } }

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
fn errno() -> i32 { unsafe { *libc::__error() } }

#[cfg(target_os = "dragonfly")]
fn errno() -> i32 { unsafe { *libc::__dfly_error() } }

fn write_errno(msg: &str, errno: i32) {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    let _ = stderr.write(msg.as_bytes());
    let _ = stderr.write(unsafe { CStr::from_ptr(strerror(errno)) }.to_bytes());
    let _ = stderr.write_all(b"\n");
}

pub(crate) fn geteuid() -> io::Result<u32> { Ok(unsafe { libc::geteuid() } as u32) }

pub(crate) fn getuid() -> io::Result<u32> { Ok(unsafe { libc::getuid() } as u32) }

pub(crate) fn is_root() -> bool { unsafe { libc::geteuid() == 0 } }

pub unsafe fn fork() -> io::Result<u32> { cvt(libc::fork()).map(|pid| pid as u32) }

pub fn wait_for_interrupt(pid: u32) -> io::Result<()> {
    let mut status;

    loop {
        status = 0;
        match unsafe { waitpid(pid as i32, &mut status, WUNTRACED) } {
            -1 if errno() == EINTR => continue,
            -1 => break Err(io::Error::from_raw_os_error(errno())),
            _ => break Ok(()),
        }
    }
}

pub fn wait_for_child(pid: u32) -> io::Result<u8> {
    let mut status;
    let mut result;

    loop {
        status = 0;
        result = unsafe { waitpid(pid as i32, &mut status, WUNTRACED) };
        if result == -1 {
            break if errno() == ECHILD {
                Ok(unsafe { WEXITSTATUS(status) as u8 })
            } else {
                Err(io::Error::from_raw_os_error(errno()))
            };
        }
    }
}

pub fn fork_exit(exit_status: i32) -> ! { unsafe { libc::_exit(exit_status) } }

pub(crate) fn getpid() -> io::Result<u32> { cvt(unsafe { libc::getpid() }).map(|pid| pid as u32) }

pub(crate) fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(pid as pid_t, signal as c_int) }).and(Ok(()))
}

pub(crate) fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(-(pgid as pid_t), signal as c_int) }).and(Ok(()))
}

pub(crate) fn fork_and_exec<F: Fn()>(
    prog: &str,
    args: &[&str],
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    clear_env: bool,
    before_exec: F,
) -> io::Result<u32> {
    let prog_str = match CString::new(prog) {
        Ok(prog) => prog,
        Err(_) => {
            return Err(io::Error::last_os_error());
        }
    };

    // Create a vector of null-terminated strings.
    let mut cvt_args: Vec<CString> = Vec::new();
    cvt_args.push(prog_str.clone());
    for &arg in args.iter() {
        match CString::new(arg) {
            Ok(arg) => cvt_args.push(arg),
            Err(_) => {
                return Err(io::Error::last_os_error());
            }
        }
    }

    // Create a null-terminated array of pointers to those strings.
    let mut arg_ptrs: Vec<*const c_char> = cvt_args.iter().map(|x| x.as_ptr()).collect();
    arg_ptrs.push(ptr::null());

    // Get the PathBuf of the program if it exists.
    let prog = if prog.contains('/') {
        // This is a fully specified path to an executable.
        Some(prog_str)
    } else if let Ok(paths) = env::var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        env::split_paths(&paths)
            .filter_map(|mut path| {
                path.push(prog);
                match (path.exists(), path.to_str()) {
                    (true, Some(path)) => CString::new(path).ok(),
                    _ => None,
                }
            })
            .next()
    } else {
        None
    };

    let mut env_ptrs: Vec<*const c_char> = Vec::new();
    let mut env_vars: Vec<CString> = Vec::new();

    // If clear_env is not specified build envp
    if !clear_env {
        for (key, value) in env::vars() {
            match CString::new(format!("{}={}", key, value)) {
                Ok(var) => env_vars.push(var),
                Err(_) => {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        env_ptrs = env_vars.iter().map(|x| x.as_ptr()).collect();
    }
    env_ptrs.push(ptr::null());

    if let Some(prog) = prog {
        unsafe {
            match fork()? {
                0 => {
                    if let Some(stdin) = stdin {
                        let _ = dup2(stdin, STDIN_FILENO);
                        let _ = close(stdin);
                    }

                    if let Some(stdout) = stdout {
                        let _ = dup2(stdout, STDOUT_FILENO);
                        let _ = close(stdout);
                    }

                    if let Some(stderr) = stderr {
                        let _ = dup2(stderr, STDERR_FILENO);
                        let _ = close(stderr);
                    }

                    before_exec();

                    libc::execve(prog.as_ptr(), arg_ptrs.as_ptr(), env_ptrs.as_ptr());
                    eprintln!("ion: command exec: {}", io::Error::last_os_error());
                    fork_exit(1);
                }
                pid => {
                    if let Some(stdin) = stdin {
                        let _ = close(stdin);
                    }

                    if let Some(stdout) = stdout {
                        let _ = close(stdout);
                    }

                    if let Some(stderr) = stderr {
                        let _ = close(stderr);
                    }

                    Ok(pid)
                }
            }
        }
    } else {
        Err(io::Error::from_raw_os_error(libc::ENOENT))
    }
}

pub(crate) fn execve(prog: &str, args: &[String], clear_env: bool) -> io::Error {
    let prog_str = match CString::new(prog) {
        Ok(prog) => prog,
        Err(_) => {
            return io::Error::last_os_error();
        }
    };

    // Create a vector of null-terminated strings.
    let mut cvt_args: Vec<CString> = Vec::new();
    cvt_args.push(prog_str.clone());
    for arg in args.iter() {
        match CString::new(&**arg) {
            Ok(arg) => cvt_args.push(arg),
            Err(_) => {
                return io::Error::last_os_error();
            }
        }
    }

    // Create a null-terminated array of pointers to those strings.
    let mut arg_ptrs: Vec<*const c_char> = cvt_args.iter().map(|x| x.as_ptr()).collect();
    arg_ptrs.push(ptr::null());

    // Get the PathBuf of the program if it exists.
    let prog = if prog.contains('/') {
        // This is a fully specified path to an executable.
        Some(prog_str)
    } else if let Ok(paths) = env::var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        env::split_paths(&paths)
            .filter_map(|mut path| {
                path.push(prog);
                match (path.exists(), path.to_str()) {
                    (true, Some(path)) => CString::new(path).ok(),
                    _ => None,
                }
            })
            .next()
    } else {
        None
    };

    let mut env_ptrs: Vec<*const c_char> = Vec::new();
    let mut env_vars: Vec<CString> = Vec::new();

    // If clear_env is not specified build envp
    if !clear_env {
        for (key, value) in env::vars() {
            match CString::new(format!("{}={}", key, value)) {
                Ok(var) => env_vars.push(var),
                Err(_) => {
                    return io::Error::last_os_error();
                }
            }
        }
        env_ptrs = env_vars.iter().map(|x| x.as_ptr()).collect();
    }
    env_ptrs.push(ptr::null());

    if let Some(prog) = prog {
        // If we found the program. Run it!
        unsafe { libc::execve(prog.as_ptr(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };
        io::Error::last_os_error()
    } else {
        // The binary was not found.
        io::Error::from_raw_os_error(libc::ENOENT)
    }
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
    use users_unix::{get_user_by_name, os::unix::UserExt};

    pub(crate) fn get_user_home(username: &str) -> Option<String> {
        match get_user_by_name(username) {
            Some(user) => Some(user.home_dir().to_string_lossy().into_owned()),
            None => None,
        }
    }
}
