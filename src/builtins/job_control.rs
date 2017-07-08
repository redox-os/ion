use shell::Shell;
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use std::io::{stderr, Write};
#[cfg(all(unix, not(target_os = "redox")))] use libc::pid_t;
#[cfg(not(target_os = "redox"))] use nix::sys::signal::{self, Signal};
#[cfg(not(target_os = "redox"))] use nix::unistd;

#[cfg(all(unix, not(target_os = "redox")))]
/// When given a process ID, that process's group will be assigned as the foreground process group.
pub fn set_foreground(pid: u32) {
    let _ = unistd::tcsetpgrp(0, pid as i32);
    let _ = unistd::tcsetpgrp(1, pid as i32);
    let _ = unistd::tcsetpgrp(2, pid as i32);
}

#[cfg(target_os = "redox")]
pub fn set_foreground(pid: u32) {
    // TODO
}

#[cfg(all(unix, not(target_os = "redox")))]
/// Suspends a given process by it's process ID.
pub fn suspend(pid: u32) {
    let _ = signal::kill(-(pid as pid_t), Some(Signal::SIGSTOP));
}

#[cfg(all(unix, not(target_os = "redox")))]
/// Resumes a given process by it's process ID.
fn resume(pid: u32) {
    let _ = signal::kill(-(pid as pid_t), Some(Signal::SIGCONT));
}

pub fn disown(shell: &mut Shell, args: &[&str]) -> i32 {
    let stderr = stderr();
    let mut stderr = stderr.lock();
    const NO_SIGHUP: u8 = 1;
    const ALL_JOBS:  u8 = 2;
    const RUN_JOBS:  u8 = 4;

    let mut jobspecs = Vec::new();
    let mut flags = 0u8;
    for &arg in args {
        match arg {
            "-a" => flags |= ALL_JOBS,
            "-h" => flags |= NO_SIGHUP,
            "-r" => flags |= RUN_JOBS,
            _    => match arg.parse::<u32>() {
                Ok(jobspec) => jobspecs.push(jobspec),
                Err(_) => {
                    let _ = writeln!(stderr, "ion: disown: invalid jobspec: '{}'", arg);
                    return FAILURE
                },
            }
        }
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
                if flags & NO_SIGHUP != 0 { process.ignore_sighup = true; }
                process.state = ProcessState::Empty;
                match jobspecs.next() {
                    Some(jobspec) => current_jobspec = jobspec,
                    None          => break
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

    SUCCESS
}


#[cfg(target_os = "redox")]
pub fn suspend(pid: u32) {
    use syscall;
    let _ = syscall::kill(pid as usize, syscall::SIGSTOP);
}

#[cfg(target_os = "redox")]
fn resume(pid: u32) {
    use syscall;
    let _ = syscall::kill(pid as usize, syscall::SIGCONT);
}

/// Display a list of all jobs running in the background.
pub fn jobs(shell: &mut Shell) {
    let stderr = stderr();
    let mut stderr = stderr.lock();
    for (id, process) in shell.background.lock().unwrap().iter().enumerate() {
        match process.state {
            ProcessState::Empty => (),
            _ => { let _ = writeln!(stderr, "[{}] {} {}", id, process.pid, process.state); }
        }
    }
}

pub fn fg(shell: &mut Shell, args: &[&str]) -> i32 {
    let mut status = 0;
    for arg in args {
        if let Ok(njob) = arg.parse::<u32>() {
            let job;
            if let Some(borrowed_job) = shell.background.lock().unwrap().iter().nth(njob as usize) {
                job = borrowed_job.clone();
            } else {
                let stderr = stderr();
                let _ = writeln!(stderr.lock(), "ion: fg: job {} does not exist", njob);
                status = FAILURE;
                continue
            }

            match job.state {
                ProcessState::Running => {
                    set_foreground(njob);
                    // TODO: This doesn't work
                    status = shell.watch_foreground(njob)
                },
                ProcessState::Stopped => {
                    resume(job.pid);
                    set_foreground(njob);
                    // TODO: This doesn't work
                    status = shell.watch_foreground(njob);
                },
                ProcessState::Empty => {
                    let stderr = stderr();
                    let _ = writeln!(stderr.lock(), "ion: fg: job {} does not exist", njob);
                    status = FAILURE;
                }
            }
        } else {
            let stderr = stderr();
            let _ = writeln!(stderr.lock(), "ion: fg: {} is not a valid job number", arg);
        }
    }
    status
}

pub fn bg(shell: &mut Shell, args: &[&str]) -> i32 {
    let mut error = false;
    let stderr = stderr();
    let mut stderr = stderr.lock();
    for arg in args {
        if let Ok(njob) = arg.parse::<u32>() {
            if let Some(job) = shell.background.lock().unwrap().iter_mut().nth(njob as usize) {
                match job.state {
                    ProcessState::Running => {
                        let _ = writeln!(stderr, "ion: bg: job {} is already running", njob);
                        error = true;
                    },
                    ProcessState::Stopped => {
                        resume(job.pid);
                        job.state = ProcessState::Running;
                        let _ = writeln!(stderr, "[{}] {} Running", njob, job.pid);
                    },
                    ProcessState::Empty => {
                        let _ = writeln!(stderr, "ion: bg: job {} does not exist", njob);
                        error = true;
                    }
                }
            } else {
                let _ = writeln!(stderr, "ion: bg: job {} does not exist", njob);
                error = true;
            }
        } else {
            let _ = writeln!(stderr, "ion: bg: {} is not a valid job number", arg);
            error = true;
        }
    }
    if error { FAILURE } else { SUCCESS }
}
