//! Contains the `jobs`, `disown`, `bg`, and `fg` commands that manage job control in the shell.

use std::error::Error;
use shell::Shell;
use shell::job_control::{JobControl, ProcessState};
use shell::signals;
use shell::status::*;
use std::io::{stderr, stdout, Write};

const DISOWN_MAN_PAGE: &'static str = r#"NAME
    disown - Disown processes

SYNOPSIS
    disown [ --help | -r | -h | -a ][PID...]

DESCRIPTION
    Disowning a process removes that process from the shell's background process table.

OPTIONS
    -r  Remove all running jobs from the background process list.
    -h  Specifies that each job supplied will not receive the SIGHUP signal when the shell receives a SIGHUP.
    -a  If no job IDs were supplied, remove all jobs from the background process list.
"#;

/// Disowns given process job IDs, and optionally marks jobs to not receive SIGHUP signals.
/// The `-a` flag selects all jobs, `-r` selects all running jobs, and `-h` specifies to mark
/// SIGHUP ignoral.
pub(crate) fn disown(shell: &mut Shell, args: &[&str]) -> Result<(), String> {
    fn print_help(ret: Result<(), String>) -> Result<(), String> {
        let stdout = stdout();
        let mut stdout = stdout.lock();

        return match stdout.write_all(DISOWN_MAN_PAGE.as_bytes()).and_then(|_| stdout.flush()) {
            Ok(_) => ret,
            Err(err) => Err(err.description().to_owned()),
        }
    }

    const NO_SIGHUP: u8 = 1;
    const ALL_JOBS: u8 = 2;
    const RUN_JOBS: u8 = 4;

    let mut jobspecs = Vec::new();
    let mut flags = 0u8;
    for &arg in args {
        match arg {
            "-a" => flags |= ALL_JOBS,
            "-h" => flags |= NO_SIGHUP,
            "-r" => flags |= RUN_JOBS,
            "--help" => {
                return print_help(Ok(()));
            },
            _ => match arg.parse::<u32>() {
                Ok(jobspec) => jobspecs.push(jobspec),
                Err(_) => {
                    return Err(format!("invalid jobspec: '{}'", arg));
                }
            },
        }
    }

    if flags == 0 {
        return print_help(Err("must provide arguments".to_owned()));
    } else if (flags & ALL_JOBS) == 0 && jobspecs.is_empty() {
        return Err("must provide a jobspec with -h or -r".to_owned());
    }

    let mut processes = shell.background.lock().unwrap();
    if jobspecs.is_empty() && flags & ALL_JOBS != 0 {
        if flags & NO_SIGHUP != 0 {
            for process in processes.iter_mut() {
                process.ignore_sighup = true;
            }
        } else {
            for process in processes.iter_mut() {
                process.state = ProcessState::Empty;
            }
        }
    } else {
        jobspecs.sort();

        let mut jobspecs = jobspecs.into_iter();
        let mut current_jobspec = jobspecs.next().unwrap();
        for (id, process) in processes.iter_mut().enumerate() {
            if id == current_jobspec as usize {
                if flags & NO_SIGHUP != 0 {
                    process.ignore_sighup = true;
                }
                process.state = ProcessState::Empty;
                match jobspecs.next() {
                    Some(jobspec) => current_jobspec = jobspec,
                    None => break,
                }
            }
        }

        if flags & RUN_JOBS != 0 {
            for process in processes.iter_mut() {
                if process.state == ProcessState::Running {
                    process.state = ProcessState::Empty;
                }
            }
        }
    }

    Ok(())
}

/// Display a list of all jobs running in the background.
pub(crate) fn jobs(shell: &mut Shell) {
    let stderr = stderr();
    let mut stderr = stderr.lock();
    for (id, process) in shell.background.lock().unwrap().iter().enumerate() {
        if process.state != ProcessState::Empty {
            let _ =
                writeln!(stderr, "[{}] {} {}\t{}", id, process.pid, process.state, process.name);
        }
    }
}

/// Hands control of the foreground process to the specified jobs, recording their exit status.
/// If the job is stopped, the job will be resumed.
/// If multiple jobs are given, then only the last job's exit status will be returned.
pub(crate) fn fg(shell: &mut Shell, args: &[&str]) -> i32 {
    fn fg_job(shell: &mut Shell, njob: u32) -> i32 {
        let job;
        if let Some(borrowed_job) = shell.background.lock().unwrap().iter().nth(njob as usize) {
            job = borrowed_job.clone();
        } else {
            let stderr = stderr();
            let _ = writeln!(stderr.lock(), "ion: fg: job {} does not exist", njob);
            return FAILURE;
        }

        // Bring the process into the foreground and wait for it to finish.
        match job.state {
            // Give the bg task the foreground, and wait for it to finish.
            ProcessState::Running => shell.set_bg_task_in_foreground(job.pid, false),
            // Same as above, but also resumes the stopped process in advance.
            ProcessState::Stopped => shell.set_bg_task_in_foreground(job.pid, true),
            // Informs the user that the specified job ID no longer exists.
            ProcessState::Empty => {
                let stderr = stderr();
                let _ = writeln!(stderr.lock(), "ion: fg: job {} does not exist", njob);
                FAILURE
            }
        }
    }

    let mut status = 0;
    if args.is_empty() {
        if shell.previous_job == !0 {
            eprintln!("ion: fg: no jobs are running in the background");
            status = FAILURE;
        } else {
            let previous_job = shell.previous_job;
            status = fg_job(shell, previous_job);
        }
    } else {
        for arg in args {
            match arg.parse::<u32>() {
                Ok(njob) => status = fg_job(shell, njob),
                Err(_) => {
                    let stderr = stderr();
                    let _ = writeln!(stderr.lock(), "ion: fg: {} is not a valid job number", arg);
                    status = FAILURE;
                }
            }
        }
    }
    status
}

/// Resumes a stopped background process, if it was stopped.
pub(crate) fn bg(shell: &mut Shell, args: &[&str]) -> i32 {
    fn bg_job(shell: &mut Shell, njob: u32) -> bool {
        if let Some(job) = shell.background.lock().unwrap().iter_mut().nth(njob as usize) {
            match job.state {
                ProcessState::Running => {
                    eprintln!("ion: bg: job {} is already running", njob);
                    return true;
                }
                ProcessState::Stopped => signals::resume(job.pid),
                ProcessState::Empty => {
                    eprintln!("ion: bg: job {} does not exist", njob);
                    return true;
                }
            }
        } else {
            eprintln!("ion: bg: job {} does not exist", njob);
            return true;
        }
        false
    }

    let mut error = false;
    if args.is_empty() {
        if shell.previous_job == !0 {
            eprintln!("ion: bg: no jobs are running in the background");
            error = true;
        } else {
            let previous_job = shell.previous_job;
            error = bg_job(shell, previous_job);
        }
    } else {
        for arg in args {
            error = if let Ok(njob) = arg.parse::<u32>() {
                bg_job(shell, njob)
            } else {
                eprintln!("ion: bg: {} is not a valid job number", arg);
                true
            };
        }
    }
    if error {
        FAILURE
    } else {
        SUCCESS
    }
}
