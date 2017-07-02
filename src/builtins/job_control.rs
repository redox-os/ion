use shell::Shell;
use shell::job_control::ProcessState;
use shell::status::*;
use std::io::{stderr, Write};
#[cfg(not(target_os = "redox"))] use nix::sys::signal::{self, Signal};
#[cfg(not(target_os = "redox"))] use libc::pid_t;

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
pub fn bg(shell: &mut Shell, args: &[&str]) -> i32 {
    let mut error = false;
    let stderr = stderr();
    let mut stderr = stderr.lock();
    for arg in args {
        if let Ok(njob) = arg.parse::<u32>() {
            if let Some(job) = shell.background.lock().unwrap().iter_mut().nth(njob as usize) {
                if let ProcessState::Running = job.state {
                    let _ = writeln!(stderr, "ion: bg: job {} is already running", njob);
                    error = true;
                } else {
                    let _ = signal::kill(job.pid as pid_t, Some(Signal::SIGCONT));
                    job.state = ProcessState::Running;
                    let _ = writeln!(stderr, "[{}] {} {}", njob, job.pid, job.state);
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
pub fn bg(_: &mut Shell, _: &[&str]) {
    let stderr = stderr();
    // TODO: Redox signal handling support
    let _ = writeln!(stderr.lock(), "Redox does not yet support signals");
}
