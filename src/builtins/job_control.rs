use shell::Shell;
use shell::job_control::{JobControl, ProcessState, resume};
use shell::status::*;
use std::io::{stderr, Write};

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

            // Bring the process into the foreground and wait for it to finish.
            status = match job.state {
                ProcessState::Running => shell.set_bg_task_in_foreground(job.pid, false),
                ProcessState::Stopped => shell.set_bg_task_in_foreground(job.pid, true),
                ProcessState::Empty => {
                    let stderr = stderr();
                    let _ = writeln!(stderr.lock(), "ion: fg: job {} does not exist", njob);
                    FAILURE
                }
            };
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
                    ProcessState::Stopped => resume(job.pid),
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
