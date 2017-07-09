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
    pub fn ion_fork() -> syscall::error::Result<Fork> {
        use syscall::call::clone;
        unsafe {
            clone(0).map(|pid| {
                if pid == 0 { Fork::Child } else { Fork::Parent(pid as u32) }
            })
        }
    }

    /// Ensures that the forked child is given a unique process ID.
    pub fn create_process_group() {

    }
}


#[cfg(all(unix, not(target_os = "redox")))]
mod unix {
    use nix::Error as NixError;
    use nix::unistd::{fork, ForkResult, setpgid};
    use super::Fork;
    pub fn ion_fork() -> Result<Fork, NixError> {
        match fork()? {
            ForkResult::Parent{ child: pid } => Ok(Fork::Parent(pid as u32)),
            ForkResult::Child                => Ok(Fork::Child)
        }
    }

    /// Ensures that the forked child is given a unique process ID.
    pub fn create_process_group() {
        let _ = setpgid(0, 0);
    }
}

use std::process::{Command, exit};
use super::job::JobKind;
use super::job_control::{JobControl, ProcessState};
use super::Shell;
use super::signals;
use super::status::*;
use super::pipe::pipe;

pub fn fork_pipe(shell: &mut Shell, commands: Vec<(Command, JobKind)>) -> i32 {
    match ion_fork() {
        Ok(Fork::Parent(pid)) => {
            shell.send_to_background(pid, ProcessState::Running);
            SUCCESS
        },
        Ok(Fork::Child) => {
            signals::unblock();
            create_process_group();
            exit(pipe(shell, commands, false));
        },
        Err(why) => {
            eprintln!("ion: background fork failed: {}", why);
            FAILURE
        }
    }
}
