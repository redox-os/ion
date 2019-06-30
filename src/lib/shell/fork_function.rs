use super::{fork::IonResult, variables::Value, Capture, Shell};
use nix::unistd::{self, Pid};

impl<'a> Shell<'a> {
    /// High-level function for executing a function programmatically.
    /// NOTE: Always add "ion" as a first argument in `args`.
    pub fn fork_function<S: AsRef<str>, T, F: FnOnce(IonResult) -> Result<T, ()>>(
        &self,
        capture: Capture,
        result: F,
        fn_name: &str,
        args: &[S],
    ) -> Result<T, ()> {
        if let Some(Value::Function(function)) = self.variables.get(fn_name) {
            let output = self
                .fork(capture, move |child| {
                    if let Err(err) = function.execute(child, args) {
                        if capture == Capture::None {
                            eprintln!("ion: {} function call: {}", fn_name, err);
                        }
                    }
                    Ok(())
                })
                .map_err(|err| eprintln!("ion: fork error: {}", err))
                .and_then(result);

            // Ensure that the parent retains ownership of the terminal before exiting.
            let _ = unistd::tcsetpgrp(nix::libc::STDIN_FILENO, Pid::this());
            output
        } else {
            Err(())
        }
    }
}
