use sys;

/// Ensures that the forked child is given a unique process ID.
pub(crate) fn create_process_group(pgid: u32) { let _ = sys::setpgid(0, pgid); }

use super::job_control::{JobControl, ProcessState};
use super::pipe;
use super::super::Shell;
use super::super::job::{JobKind, RefinedJob};
use super::super::status::*;
use std::process::exit;

/// Forks the shell, adding the child to the parent's background list, and executing
/// the given commands in the child fork.
pub(crate) fn fork_pipe(
    shell: &mut Shell,
    commands: Vec<(RefinedJob, JobKind)>,
    command_name: String,
) -> i32 {
    match unsafe { sys::fork() } {
        Ok(0) => {
            shell.is_background_shell = true;
            let _ = sys::reset_signal(sys::SIGINT);
            let _ = sys::reset_signal(sys::SIGHUP);
            let _ = sys::reset_signal(sys::SIGTERM);
            // This ensures that the child fork has a unique PGID.
            create_process_group(0);
            sys::close_stdin();
            // After execution of it's commands, exit with the last command's status.
            exit(pipe(shell, commands, false));
        }
        Ok(pid) => {
            // The parent process should add the child fork's PID to the background.
            shell.send_to_background(pid, ProcessState::Running, command_name);
            SUCCESS
        }
        Err(why) => {
            eprintln!("ion: background fork failed: {}", why);
            exit(FAILURE);
        }
    }
}
