extern crate libc;
extern crate syscall;

use std::{
    env::{split_paths, var, vars},
    ffi::OsStr,
    fs::File,
    io::{self, prelude::*, BufReader, SeekFrom},
    mem,
    os::unix::{
        ffi::OsStrExt,
        fs::MetadataExt,
        io::{AsRawFd, RawFd},
    },
    path::PathBuf,
    process::exit,
    slice,
};
use syscall::{waitpid as waitpid_, SigAction};
pub use syscall::{
    wcoredump as wcoredump_, wexitstatus as wexitstatus_, wifcontinued as wifcontinued_,
    wifexited as wifexited_, wifsignaled as wifsignaled_, wifstopped as wifstopped_,
    wstopsig as wstopsig_, wtermsig as wtermsig_, ECHILD, EINTR,
};

pub const NULL_PATH: &str = "null:";

pub fn fork_and_exec<F: Fn(), S: AsRef<str>>(
    prog: &str,
    args: &[S],
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    clear_env: bool,
    before_exec: F,
) -> io::Result<u32> {
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

                let error = execve(prog, args, clear_env);
                eprintln!("ion: command exec: {}", error);
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
}

pub fn execve<S: AsRef<str>>(prog: &str, args: &[S], clear_env: bool) -> io::Error {
    // Get the PathBuf of the program if it exists.
    let prog = if prog.contains(':') || prog.contains('/') {
        // This is a fully specified scheme or path to an
        // executable.
        Some(PathBuf::from(prog))
    } else if let Ok(paths) = var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        split_paths(&paths)
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

    if let Some(prog) = prog {
        let mut file = match File::open(&prog) {
            Ok(file) => file,
            Err(err) => return err,
        };

        // Construct a valid set of arguments to pass to execve. Ensure that
        // the interpreter program is the first argument, if any. Then comes
        // the program name. Finally all the arguments.
        let mut cvt_args: Vec<[usize; 2]> = Vec::with_capacity(args.len());

        // Check the interpreter.
        // `./test.ion` with `#!/bin/ion` should become:
        // /bin/ion ./test.ion <args...>
        let interpreter = {
            let mut reader = BufReader::new(&file);

            let mut shebang = [0; 2];
            let mut read = 0;
            while read < shebang.len() {
                match reader.read(&mut shebang[read..]) {
                    Ok(0) => break,
                    Ok(n) => read += n,
                    Err(err) => return err,
                }
            }

            if &shebang == b"#!" {
                // This should be interpreted.
                // Since fexec won't be called on the file itself but rather the interpreter,
                // we need to make sure ourselves the file is executable
                let metadata = match file.metadata() {
                    Ok(meta) => meta,
                    Err(err) => return err,
                };

                let uid = match syscall::getuid() {
                    Ok(uid) => uid,
                    Err(err) => return io::Error::from_raw_os_error(err.errno),
                };
                let gid = match syscall::getgid() {
                    Ok(gid) => gid,
                    Err(err) => return io::Error::from_raw_os_error(err.errno),
                };
                let mode = if uid == metadata.uid() as usize {
                    (metadata.mode() >> 3 * 2) & 0o7
                } else if gid == metadata.gid() as usize {
                    (metadata.mode() >> 3 * 1) & 0o7
                } else {
                    metadata.mode() & 0o7
                };

                if mode & 0o1 == 0o0 {
                    return io::ErrorKind::PermissionDenied.into();
                }

                let mut interpreter = Vec::new();
                match reader.read_until(b'\n', &mut interpreter) {
                    Ok(_) => {
                        if interpreter.ends_with(&[b'\n']) {
                            interpreter.pop().unwrap();
                        }
                        // TODO: When NLL becomes stable, reassign `file =`
                        // directly here instead of the `let interpreter = {`
                        // workaround.
                        // (But remember to make sure the vector lives long
                        // enough for the arguments!!)
                        Some(interpreter)
                    }
                    Err(err) => return err,
                }
            } else {
                match reader.seek(SeekFrom::Start(0)) {
                    Ok(_) => (),
                    Err(err) => return err,
                }
                None
            }
        };
        if let Some(ref interpreter) = interpreter {
            let path: &OsStr = OsStrExt::from_bytes(&interpreter);
            file = match File::open(path) {
                Ok(file) => file,
                Err(err) => return err,
            };
            cvt_args.push([interpreter.as_ptr() as usize, interpreter.len()]);
        }

        // Push the program name
        cvt_args.push([prog.as_os_str().as_bytes().as_ptr() as usize, prog.as_os_str().len()]);

        // Push all arguments
        for arg in args {
            let arg: &str = arg.as_ref();
            cvt_args.push([arg.as_ptr() as usize, arg.len()]);
        }

        // Push all environment variables
        let mut env_args: Vec<[usize; 2]> = Vec::new();
        let mut env_key_value: Vec<String> = Vec::new();
        if !clear_env {
            for (key, value) in vars() {
                env_key_value.push(key + "=" + &value);
            }
            // Can't use the same loop because pushing to a vector may reallocate.
            for env in &env_key_value {
                env_args.push([env.as_ptr() as usize, env.len()]);
            }
        }

        // Finally: Run the program!
        let error = syscall::fexec(file.as_raw_fd(), &cvt_args, &env_args);
        io::Error::from_raw_os_error(error.err().unwrap().errno)
    } else {
        // The binary was not found.
        io::Error::from_raw_os_error(syscall::ENOENT)
    }
}
