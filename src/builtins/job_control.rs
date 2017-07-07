use shell::Shell;
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use std::io::{stderr, Write};
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

#[cfg(all(unix, not(target_os = "redox")))]
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
                    let _ = signal::kill(-(job.pid as i32), Some(Signal::SIGCONT));
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

#[cfg(target_os = "redox")]
pub fn fg(_: &mut Shell, _: &[&str]) -> i32 {
    let stderr = stderr();
    // TODO: Redox signal handling support
    let _ = writeln!(stderr.lock(), "Redox does not yet support signals");
    0
}

#[cfg(all(unix, not(target_os = "redox")))]
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
                        let _ = signal::kill(-(job.pid as i32), Some(Signal::SIGCONT));
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

#[cfg(target_os = "redox")]
pub fn bg(_: &mut Shell, _: &[&str]) -> i32 {
    let stderr = stderr();
    // TODO: Redox signal handling support
    let _ = writeln!(stderr.lock(), "Redox does not yet support signals");
    0
}
