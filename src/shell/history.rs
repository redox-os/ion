use super::Shell;
use super::status::*;

use regex::Regex;
use std::io::{self, Write};
use types::Array;

bitflags! {
    struct IgnoreFlags: u8 {
        // Macro definition fails if last flag has a comment at the end of the line.
        /// ignore all commands ("all")
        const ALL                = (0b1 << 0);
        /// ignore commands with leading whitespace ("whitespace")
        const WHITESPACE         = (0x1 << 1);
        /// ignore commands with status code 127 ("no_such_command")
        const NO_SUCH_COMMAND    = (0b1 << 2);
        /// used if regexes are defined.
        const BASED_ON_REGEX     = (0b1 << 3);
    }
}

pub(crate) struct IgnoreSetting {
    flags: IgnoreFlags,
    // Yes, a bad heap-based Vec, however unfortunately its not possible to store Regex'es in Array
    regexes: Option<Vec<Regex>>,
}

impl IgnoreSetting {
    pub(crate) fn default() -> IgnoreSetting {
        IgnoreSetting {
            flags:   IgnoreFlags::empty(),
            regexes: None,
        }
    }
}

/// Contains all history-related functionality for the `Shell`.
pub(crate) trait ShellHistory {
    /// Prints the commands contained within the history buffers to standard output.
    fn print_history(&self, _arguments: &[&str]) -> i32;

    /// Sets the history size for the shell context equal to the HISTORY_SIZE shell variable if
    /// it
    /// is set otherwise to a default value (1000).
    ///
    /// If the HISTFILE_ENABLED shell variable is set to 1, then HISTFILE_SIZE is synced
    /// with the shell context as well. Otherwise, the history file name is set to None in the
    /// shell context.
    ///
    /// This is called in on_command so that the history length and history file state will be
    /// updated correctly after a command is entered that alters them and just before loading
    /// the
    /// history file so that it will be loaded correctly.
    fn set_context_history_from_vars(&mut self);

    /// Saves a command in the history, depending on @HISTORY_IGNORE. Should be called
    /// immediately after `on_command()`
    fn save_command_in_history(&mut self, command: &str);

    /// Updates the history ignore patterns. Call this whenever HISTORY_IGNORE is changed.
    fn update_ignore_patterns(&mut self, patterns: &Array);
}

trait ShellHistoryPrivate {
    /// Returns true if the given command with the given exit status should be saved in the
    /// history
    fn should_save_command(&self, command: &str) -> bool;
}

impl ShellHistory for Shell {
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
        let max_history_size =
            self.variables.get_var_or_empty("HISTORY_SIZE").parse().unwrap_or(1000);

        context.history.set_max_size(max_history_size);

        if &*self.variables.get_var_or_empty("HISTFILE_ENABLED") == "1" {
            let file_name = self.variables.get_var("HISTFILE");
            context.history.set_file_name(file_name.map(|f| f.into()));

            let max_histfile_size =
                self.variables.get_var_or_empty("HISTFILE_SIZE").parse().unwrap_or(1000);
            context.history.set_max_file_size(max_histfile_size);
        } else {
            context.history.set_file_name(None);
        }
    }

    fn save_command_in_history(&mut self, command: &str) {
        if self.should_save_command(command) {
            // Mark the command in the context history
            self.set_context_history_from_vars();
            if let Err(err) = self.context.as_mut().unwrap().history.push(command.into()) {
                eprintln!("ion: {}", err);
            }
        }
    }

    fn update_ignore_patterns(&mut self, patterns: &Array) {
        let mut flags = IgnoreFlags::empty();
        let mut regexes = Vec::new();
        // for convenience and to avoid typos
        let regex_prefix = "regex:";
        for pattern in patterns {
            match pattern.as_ref() {
                "all" => flags |= IgnoreFlags::ALL,
                "no_such_command" => flags |= IgnoreFlags::NO_SUCH_COMMAND,
                "whitespace" => flags |= IgnoreFlags::WHITESPACE,
                // The length check is there to just ignore empty regex definitions
                _ if pattern.starts_with(regex_prefix) && pattern.len() > regex_prefix.len() => {
                    flags |= IgnoreFlags::BASED_ON_REGEX;
                    let regex_string = &pattern[regex_prefix.len()..];
                    // We save the compiled regexes, as compiling them can be  an expensive task
                    if let Ok(regex) = Regex::new(regex_string) {
                        regexes.push(regex);
                    }
                }
                _ => continue,
            }
        }

        self.ignore_setting.flags = flags;
        self.ignore_setting.regexes = if regexes.len() > 0 { Some(regexes) } else { None }
    }
}

impl ShellHistoryPrivate for Shell {
    fn should_save_command(&self, command: &str) -> bool {
        // just for convenience and to make the code look a bit cleaner
        let ignore = &self.ignore_setting.flags;
        let regexes = &self.ignore_setting.regexes;

        // without the second check the command which sets the local variable would also be
        // ignored. However, this behavior might not be wanted.
        if ignore.contains(IgnoreFlags::ALL) && !command.contains("HISTORY_IGNORE") {
            return false;
        }

        // Here we allow to also ignore the setting of the local variable because we assume
        // the user entered the leading whitespace on purpose.
        if ignore.contains(IgnoreFlags::WHITESPACE) {
            if command.chars().next().map_or(false, |b| b.is_whitespace()) {
                return false;
            }
        }

        if ignore.contains(IgnoreFlags::NO_SUCH_COMMAND) && self.previous_status == NO_SUCH_COMMAND
        {
            return false;
        }

        if let Some(ref regexes) = *regexes {
            // ignore command when regex is matched but only if it does not contain
            // "HISTORY_IGNORE", otherwise we would also ignore the command which
            // sets the variable, which could be annoying.
            if regexes.iter().any(|regex| regex.is_match(command))
                && !command.contains("HISTORY_IGNORE")
            {
                return false;
            }
        }

        // default to true, as it's more likely that we want to save a command in history
        true
    }
}
