use std::io::{self, Write};
use status::*;
use super::Shell;

/// Contains all history-related functionality for the `Shell`.
pub trait ShellHistory {
    /// Prints the commands contained within the history buffers to standard output.
    fn print_history(&self, _arguments: &[String]) -> i32;

    /// Sets the history size for the shell context equal to the HISTORY_SIZE shell variable if it
    /// is set otherwise to a default value (1000).
    ///
    /// If the HISTORY_FILE_ENABLED shell variable is set to 1, then HISTORY_FILE_SIZE is synced
    /// with the shell context as well. Otherwise, the history file name is set to None in the
    /// shell context.
    ///
    /// This is called in on_command so that the history length and history file state will be
    /// updated correctly after a command is entered that alters them and just before loading the
    /// history file so that it will be loaded correctly.
    fn set_context_history_from_vars(&mut self);
}

impl<'a> ShellHistory for Shell<'a> {
    fn print_history(&self, _arguments: &[String]) -> i32 {
        let mut buffer = Vec::with_capacity(8*1024);
        for command in &self.context.history.buffers {
            let _ = writeln!(buffer, "{}", command);
        }
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        let _ = stdout.write_all(&buffer);
        SUCCESS
    }

    fn set_context_history_from_vars(&mut self) {
        let max_history_size = self.variables
            .get_var_or_empty("HISTORY_SIZE")
            .parse()
            .unwrap_or(1000);

        self.context.history.set_max_size(max_history_size);

        if self.variables.get_var_or_empty("HISTORY_FILE_ENABLED") == "1" {
            let file_name = self.variables.get_var("HISTORY_FILE");
            self.context.history.set_file_name(file_name);

            let max_history_file_size = self.variables
                .get_var_or_empty("HISTORY_FILE_SIZE")
                .parse()
                .unwrap_or(1000);
            self.context.history.set_max_file_size(max_history_file_size);
        } else {
            self.context.history.set_file_name(None);
        }
    }
}
