use super::Shell;
use super::status::*;
use std::io::{self, Write};

/// Contains all history-related functionality for the `Shell`.
pub trait ShellHistory {
    /// Prints the commands contained within the history buffers to standard output.
    fn print_history(&self, _arguments: &[&str]) -> i32;

    /// Sets the history size for the shell context equal to the HISTORY_SIZE shell variable if it
    /// is set otherwise to a default value (1000).
    ///
    /// If the HISTFILE_ENABLED shell variable is set to 1, then HISTFILE_SIZE is synced
    /// with the shell context as well. Otherwise, the history file name is set to None in the
    /// shell context.
    ///
    /// This is called in on_command so that the history length and history file state will be
    /// updated correctly after a command is entered that alters them and just before loading the
    /// history file so that it will be loaded correctly.
    fn set_context_history_from_vars(&mut self);
}

impl<'a> ShellHistory for Shell<'a> {
    fn print_history(&self, _arguments: &[&str]) -> i32 {
        if let Some(context) = self.context.as_ref() {
            let mut buffer = Vec::with_capacity(8 * 1024);
            for command in &context.history.buffers {
                let _ = writeln!(buffer, "{}", command);
            }
            let stdout = io::stdout();
            let mut stdout = stdout.lock();
            let _ = stdout.write_all(&buffer);
            SUCCESS
        } else {
            FAILURE
        }
    }

    fn set_context_history_from_vars(&mut self) {
        let context = self.context.as_mut().unwrap();
        let max_history_size = self.variables
            .get_var_or_empty("HISTORY_SIZE")
            .parse()
            .unwrap_or(1000);

        context.history.set_max_size(max_history_size);

        if &*self.variables.get_var_or_empty("HISTFILE_ENABLED") == "1" {
            let file_name = self.variables.get_var("HISTFILE");
            context.history.set_file_name(file_name.map(|f| f.into()));

            let max_histfile_size = self.variables
                .get_var_or_empty("HISTFILE_SIZE")
                .parse()
                .unwrap_or(1000);
            context.history.set_max_file_size(max_histfile_size);
        } else {
            context.history.set_file_name(None);
        }
    }
}
