use super::{
    super::{sys, Shell},
    job_control::{BackgroundProcess, ProcessState},
};
use crate::{builtins::Status, expansion::pipelines::Pipeline};

impl<'a> Shell<'a> {
    /// Ensures that the forked child is given a unique process ID.
    fn create_process_group(pgid: u32) { let _ = sys::setpgid(0, pgid); }

    /// Forks the shell, adding the child to the parent's background list, and executing
    /// the given commands in the child fork.
    pub(super) fn fork_pipe(&mut self, pipeline: Pipeline<'a>, state: ProcessState) -> Status {
        match unsafe { sys::fork() } {
            Ok(0) => {
                self.opts_mut().is_background_shell = true;
                let _ = sys::reset_signal(sys::SIGINT);
                let _ = sys::reset_signal(sys::SIGHUP);
                let _ = sys::reset_signal(sys::SIGTERM);
                let _ = sys::close(sys::STDIN_FILENO);

                // This ensures that the child fork has a unique PGID.
                Self::create_process_group(0);

                // After execution of it's commands, exit with the last command's status.
                sys::fork_exit(
                    self.pipe(pipeline)
                        .unwrap_or_else(|err| {
                            eprintln!("{}", err);
                            Status::COULD_NOT_EXEC
                        })
                        .as_os_code(),
                );
            }
            Ok(pid) => {
                if state != ProcessState::Empty {
                    // The parent process should add the child fork's PID to the background.
                    self.send_to_background(BackgroundProcess::new(
                        pid,
                        state,
                        pipeline.to_string(),
                    ));
                }
                Status::SUCCESS
            }
            Err(why) => Status::error(format!("ion: background fork failed: {}", why)),
        }
    }
}
