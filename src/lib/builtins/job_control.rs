//! Contains the `jobs`, `disown`, `bg`, and `fg` commands that manage job
//! control in the shell.

use crate::shell::{signals, status::*, BackgroundProcess, Shell};
use small;
use smallvec::SmallVec;

/// Disowns given process job IDs, and optionally marks jobs to not receive SIGHUP signals.
/// The `-a` flag selects all jobs, `-r` selects all running jobs, and `-h` specifies to mark
/// SIGHUP ignoral.
pub fn disown(shell: &mut Shell, args: &[small::String]) -> Result<(), String> {
    // Specifies that a process should be set to not receive SIGHUP signals.
    let mut no_sighup = false;
    // Specifies that all jobs in the process table should be manipulated.
    let mut all_jobs = false;
    // Specifies that only running jobs in the process table should be manipulated.
    let mut run_jobs = false;

    // Set flags and collect all job specs listed as arguments.
    let mut collected_jobs: SmallVec<[usize; 16]> = SmallVec::with_capacity(16);
    for arg in args {
        match &**arg {
            "-a" => all_jobs = true,
            "-h" => no_sighup = true,
            "-r" => run_jobs = true,
            _ => {
                let jobspec =
                    arg.parse::<usize>().map_err(|_| format!("invalid jobspec: '{}'", arg))?;
                collected_jobs.push(jobspec);
            }
        }
    }

    if !all_jobs && !run_jobs && collected_jobs.is_empty() {
        return Err("must provide arguments to select jobs".to_owned());
    } else if (all_jobs && run_jobs) || (collected_jobs.is_empty() && (all_jobs || run_jobs)) {
        return Err("must only provide a single job spec".to_owned());
    }

    let action: fn(&mut BackgroundProcess) = if no_sighup {
        |process| process.set_ignore_sighup(true)
    } else {
        |process| process.forget()
    };

    // Open the process table to access and manipulate process metadata.
    let mut process_table = shell.background.lock().unwrap();
    if all_jobs {
        process_table.iter_mut().for_each(action);
    } else if run_jobs {
        // Drop every job from the process table by setting their state to `Empty`.
        process_table.iter_mut().filter(|p| p.is_running()).for_each(action)
    } else {
        for current_jobspec in collected_jobs {
            if let Some(process) = process_table.get_mut(current_jobspec) {
                action(process);
            }
        }
    }

    Ok(())
}

/// Display a list of all jobs running in the background.
pub fn jobs(shell: &mut Shell) {
    for (id, process) in shell.background.lock().unwrap().iter().enumerate() {
        if process.exists() {
            eprintln!("[{}] {}", id, process);
        }
    }
}

/// Hands control of the foreground process to the specified jobs, recording their exit status.
/// If the job is stopped, the job will be resumed.
/// If multiple jobs are given, then only the last job's exit status will be returned.
pub fn fg(shell: &mut Shell, args: &[small::String]) -> i32 {
    fn fg_job(shell: &mut Shell, njob: u32) -> i32 {
        if let Some(job) =
            shell.background.lock().unwrap().iter().nth(njob as usize).filter(|p| p.exists())
        {
            // Give the bg task the foreground, and wait for it to finish. Also resume it if it
            // isn't running
            shell.set_bg_task_in_foreground(job.pid(), !job.is_running())
        } else {
            // Informs the user that the specified job ID no longer exists.
            eprintln!("ion: fg: job {} does not exist", njob);
            return FAILURE;
        }
    }

    let mut status = 0;
    if args.is_empty() {
        status = if let Some(previous_job) = shell.previous_job() {
            fg_job(shell, previous_job)
        } else {
            eprintln!("ion: fg: no jobs are running in the background");
            FAILURE
        };
    } else {
        for arg in args {
            match arg.parse::<u32>() {
                Ok(njob) => status = fg_job(shell, njob),
                Err(_) => {
                    eprintln!("ion: fg: {} is not a valid job number", arg);
                    status = FAILURE;
                }
            }
        }
    }
    status
}

/// Resumes a stopped background process, if it was stopped.
pub fn bg(shell: &mut Shell, args: &[small::String]) -> i32 {
    fn bg_job(shell: &mut Shell, njob: u32) -> bool {
        if let Some(job) =
            shell.background.lock().unwrap().iter().nth(njob as usize).filter(|p| p.exists())
        {
            if job.is_running() {
                eprintln!("ion: bg: job {} is already running", njob);
                false
            } else {
                signals::resume(job.pid());
                true
            }
        } else {
            eprintln!("ion: bg: job {} does not exist", njob);
            false
        }
    }

    if args.is_empty() {
        if let Some(previous_job) = shell.previous_job() {
            if bg_job(shell, previous_job) {
                SUCCESS
            } else {
                FAILURE
            }
        } else {
            eprintln!("ion: bg: no jobs are running in the background");
            FAILURE
        }
    } else {
        for arg in args {
            if let Ok(njob) = arg.parse::<u32>() {
                if !bg_job(shell, njob) {
                    return FAILURE;
                }
            } else {
                eprintln!("ion: bg: {} is not a valid job number", arg);
                return FAILURE;
            };
        }
        SUCCESS
    }
}
