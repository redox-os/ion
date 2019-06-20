use super::shared::{close, dup2, fork, fork_exit};
use libc::{c_char, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use std::{
    env::{split_paths, var, vars},
    ffi::CString,
    io,
    os::unix::io::RawFd,
    ptr,
};

pub const NULL_PATH: &str = "/dev/null";

pub fn fork_and_exec<F: Fn(), S: AsRef<str>>(
    prog: &str,
    args: &[S],
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
    for arg in args.iter() {
        match CString::new(arg.as_ref()) {
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
    } else if let Ok(paths) = var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        split_paths(&paths)
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
        for (key, value) in vars() {
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
