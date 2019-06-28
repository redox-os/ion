use super::job_control::{BackgroundProcess, ProcessState};
use crate::{
    builtins::Status,
    expansion::pipelines::Pipeline,
    shell::{RefinedJob, Shell},
};
use nix::{
    sys::signal::{self, SigHandler, Signal},
    unistd::{self, ForkResult, Pid},
};

impl<'a> Shell<'a> {
    /// Ensures that the forked child is given a unique process ID.
    fn create_process_group() { unistd::setpgid(Pid::this(), Pid::this()).unwrap(); }

    /// Forks the shell, adding the child to the parent's background list, and executing
    /// the given commands in the child fork.
    pub(super) fn fork_pipe(
        &mut self,
        pipeline: Pipeline<RefinedJob<'a>>,
        state: ProcessState,
    ) -> Status {
        match unistd::fork() {
            Ok(ForkResult::Child) => {
                self.opts_mut().is_background_shell = true;
                unsafe {
                    signal::signal(Signal::SIGINT, SigHandler::SigDfl).unwrap();
                    signal::signal(Signal::SIGHUP, SigHandler::SigDfl).unwrap();
                    signal::signal(Signal::SIGTERM, SigHandler::SigDfl).unwrap();
                }
                unistd::close(nix::libc::STDIN_FILENO).unwrap();

                // This ensures that the child fork has a unique PGID.
                Self::create_process_group();

                // After execution of it's commands, exit with the last command's status.
                let code = self
                    .pipe(pipeline)
                    .unwrap_or_else(|err| {
                        eprintln!("{}", err);
                        Status::COULD_NOT_EXEC
                    })
                    .as_os_code();
                unsafe { nix::libc::_exit(code) };
            }
            Ok(ForkResult::Parent { child }) => {
                if state != ProcessState::Empty {
                    // The parent process should add the child fork's PID to the background.
                    self.send_to_background(BackgroundProcess::new(
                        child,
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
