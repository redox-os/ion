use crate::{
    shell::{variables::Value, Capture, Shell},
    sys,
};
use std::process;

impl<'a> Shell<'a> {
    /// High-level function for executing a function programmatically.
    /// NOTE: Always add "ion" as a first argument in `args`.
    pub fn fork_function<S: AsRef<str>>(&mut self, fn_name: &str, args: &[S]) -> Result<(), ()> {
        if let Some(Value::Function(function)) = self.variables.get_ref(fn_name) {
            if let Err(err) = self.fork(Capture::None, move |child| {
                if let Err(err) = function.execute(child, args) {
                    eprintln!("ion: {} function call: {}", fn_name, err);
                }
            }) {
                eprintln!("ion: fork error: {}", err);
                Err(())
            } else {
                // Ensure that the parent retains ownership of the terminal before exiting.
                let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
                Ok(())
            }
        } else {
            Err(())
        }
    }
}
