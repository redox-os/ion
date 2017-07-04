use shell::Shell;
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use std::io::{stderr, Write};
use std::thread::sleep;
use std::time::Duration;
#[cfg(not(target_os = "redox"))] use nix::sys::signal::{self, Signal};
#[cfg(not(target_os = "redox"))] use libc::{self, pid_t};

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

#[cfg(not(target_os = "redox"))]
fn fg_listen(shell: &mut Shell, job: u32) {
    loop {
        sleep(Duration::from_millis(100));
        let job = &mut (*shell.background.lock().unwrap())[job as usize];
        if let ProcessState::Empty = job.state { break }
        if let Ok(signal) = shell.signals.try_recv() {
            match signal {
                libc::SIGTSTP => {
                    let _ = signal::kill(job.pid as pid_t, Some(Signal::SIGTSTP));
                    break
                },
                libc::SIGTERM => {
                    shell.handle_signal(libc::SIGTERM);
                },
                libc::SIGINT => {
                    let _ = signal::kill(job.pid as pid_t, Some(Signal::SIGINT));
                    break
                },
                _ => unimplemented!()
            }
        }
    }
}

#[cfg(not(target_os = "redox"))]
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
                    fg_listen(shell, njob);
                    status = SUCCESS;
                },
                ProcessState::Stopped => {
                    let _ = signal::kill(job.pid as pid_t, Some(Signal::SIGCONT));
                    fg_listen(shell, njob);
                    status = SUCCESS;
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

#[cfg(not(target_os = "redox"))]
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
                        let _ = signal::kill(job.pid as pid_t, Some(Signal::SIGCONT));
                        let _ = writeln!(stderr, "[{}] {} {}", njob, job.pid, job.state);
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
