use sys;

/// Ensures that the forked child is given a unique process ID.
pub fn create_process_group(pgid: u32) {
    let _ = sys::setpgid(0, pgid);
}

use std::process::exit;
use super::job::{RefinedJob, JobKind};
use super::job_control::{JobControl, ProcessState};
use super::Shell;
use super::signals;
use super::status::*;
use super::pipe::pipe;

/// Forks the shell, adding the child to the parent's background list, and executing
/// the given commands in the child fork.
pub fn fork_pipe (
    shell: &mut Shell,
    commands: Vec<(RefinedJob, JobKind)>,
    command_name: String
) -> i32 {
    match unsafe { sys::fork() } {
        Ok(0) => {
            // The child fork should not have any signals blocked, so the shell can control it.
            signals::unblock();
            // This ensures that the child fork has a unique PGID.
            create_process_group(0);
            // After execution of it's commands, exit with the last command's status.
            exit(pipe(shell, commands, false));
        },
        Ok(pid) => {
            // The parent process should add the child fork's PID to the background.
            shell.send_to_background(pid, ProcessState::Running, command_name);
            SUCCESS
        },
        Err(why) => {
            eprintln!("ion: background fork failed: {}", why);
            exit(FAILURE);
        }
    }
}
