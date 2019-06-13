use crate::sys;

use super::{
    super::{status::*, Shell},
    job_control::{BackgroundProcess, ProcessState},
};
use crate::parser::pipelines::Pipeline;

impl<'a> Shell<'a> {
    /// Forks the shell, adding the child to the parent's background list, and executing
    /// the given commands in the child fork.
    pub(super) fn fork_pipe(&mut self, pipeline: Pipeline<'a>, state: ProcessState) -> Status {
        match unsafe { sys::fork() } {
            Ok(0) => {
                self.opts_mut().is_background_shell = true;
                let _ = super::prepare_child(false, 0);
                let _ = sys::close(sys::STDIN_FILENO);

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
