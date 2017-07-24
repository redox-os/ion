#[cfg(target_os = "redox")]
pub use self::redox::*;
#[cfg(all(unix, not(target_os = "redox")))]
pub use self::unix::*;

pub enum Fork {
    Parent(u32),
    Child
}

#[cfg(target_os = "redox")]
mod redox {
    use syscall;
    use super::Fork;

    /// Forks the shell Redox's `clone(0)` syscall.
    pub fn ion_fork() -> syscall::error::Result<Fork> {
        use syscall::call::clone;
        unsafe {
            clone(0).map(|pid| {
                if pid == 0 { Fork::Child } else { Fork::Parent(pid as u32) }
            })
        }
    }

    /// Ensures that the forked child is given a unique process ID.
    pub fn create_process_group(pgid: u32) {

    }
}


#[cfg(all(unix, not(target_os = "redox")))]
mod unix {
    use nix::Error as NixError;
    use nix::unistd::{fork, ForkResult, setpgid};
    use super::Fork;

    /// Forks the shell using the *nix `fork()` syscall.
    pub fn ion_fork() -> Result<Fork, NixError> {
        match fork()? {
            ForkResult::Parent{ child: pid } => Ok(Fork::Parent(pid as u32)),
            ForkResult::Child                => Ok(Fork::Child)
        }
    }

    /// Ensures that the forked child is given a unique process ID.
    pub fn create_process_group(pgid: u32) {
        let _ = setpgid(0, pgid as i32);
    }
}

use std::process::{Command, exit};
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
    match ion_fork() {
        Ok(Fork::Parent(pid)) => {
            // The parent process should add the child fork's PID to the background.
            shell.send_to_background(pid, ProcessState::Running, command_name);
            SUCCESS
        },
        Ok(Fork::Child) => {
            // The child fork should not have any signals blocked, so the shell can control it.
            signals::unblock();
            // This ensures that the child fork has a unique PGID.
            create_process_group(0);
            // After execution of it's commands, exit with the last command's status.
            exit(pipe(shell, commands, false));
        },
        Err(why) => {
            eprintln!("ion: background fork failed: {}", why);
            exit(FAILURE);
        }
    }
}
