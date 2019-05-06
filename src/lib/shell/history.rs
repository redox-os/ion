use crate::shell::{status::*, Shell};

use crate::types;
use regex::Regex;
use small;
use std::{
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Default)]
pub(crate) struct IgnoreSetting {
    // Macro definition fails if last flag has a comment at the end of the line.
    /// ignore all commands ("all")
    all: bool,
    /// ignore commands with leading whitespace ("whitespace")
    whitespace: bool,
    /// ignore commands with status code 127 ("no_such_command")
    no_such_command: bool,
    /// used if regexes are defined.
    based_on_regex: bool,
    /// ignore commands that are duplicates
    duplicates: bool,
    // Yes, a bad heap-based Vec, however unfortunately its not possible to store Regex'es in Array
    regexes: Option<Vec<Regex>>,
}

/// Contains all history-related functionality for the `Shell`.
pub(crate) trait ShellHistory {
    /// Prints the commands contained within the history buffers to standard
    /// output.
    fn print_history(&self, _arguments: &[small::String]) -> i32;

    /// Saves a command in the history, depending on @HISTORY_IGNORE. Should be called
    /// immediately after `on_command()`
    fn save_command_in_history(&mut self, command: &str);

    /// Updates the history ignore patterns. Call this whenever HISTORY_IGNORE
    /// is changed.
    fn update_ignore_patterns(&mut self, patterns: &types::Array);
}

trait ShellHistoryPrivate {
    /// Returns true if the given command with the given exit status should be saved in the
    /// history
    fn should_save_command(&mut self, command: &str) -> bool;
}

impl<'a> ShellHistory for Shell<'a> {
    fn update_ignore_patterns(&mut self, patterns: &types::Array) {
        let mut settings = IgnoreSetting::default();
        let mut regexes = Vec::new();
        // for convenience and to avoid typos
        let regex_prefix = "regex:";
        for pattern in patterns {
            match pattern.as_ref() {
                "all" => settings.all = true,
                "no_such_command" => settings.no_such_command = true,
                "whitespace" => settings.whitespace = true,
                "duplicates" => settings.duplicates = true,
                // The length check is there to just ignore empty regex definitions
                _ if pattern.starts_with(regex_prefix) && pattern.len() > regex_prefix.len() => {
                    settings.based_on_regex = true;
                    let regex_string = &pattern[regex_prefix.len()..];
                    // We save the compiled regexes, as compiling them can be  an expensive task
                    if let Ok(regex) = Regex::new(regex_string) {
                        regexes.push(regex);
                    }
                }
                _ => continue,
            }
        }
        settings.regexes = if !regexes.is_empty() { Some(regexes) } else { None };

        self.ignore_setting = settings;
    }

    fn save_command_in_history(&mut self, command: &str) {
        if self.should_save_command(command) {
            if self.variables.get_str_or_empty("HISTORY_TIMESTAMP") == "1" {
                // Get current time stamp
                let since_unix_epoch =
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let cur_time_sys = ["#", &since_unix_epoch.to_owned().to_string()].concat();

                // Push current time to history
                if let Err(err) = self.context.as_mut().unwrap().history.push(cur_time_sys.into()) {
                    eprintln!("ion: {}", err)
                }
            }

            // Push command itself to history
            if let Err(err) = self.context.as_mut().unwrap().history.push(command.into()) {
                eprintln!("ion: {}", err);
            }
        }
    }

    fn print_history(&self, _arguments: &[small::String]) -> i32 {
        if let Some(context) = self.context.as_ref() {
            let mut buffer = Vec::with_capacity(8 * 1024);
            for command in &context.history {
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
}

impl<'a> ShellHistoryPrivate for Shell<'a> {
    fn should_save_command(&mut self, command: &str) -> bool {
        // just for convenience and to make the code look a bit cleaner
        let ignore = &self.ignore_setting;

        // without the second check the command which sets the local variable would
        // also be ignored. However, this behavior might not be wanted.
        if ignore.all && !command.contains("HISTORY_IGNORE") {
            return false;
        }

        // Here we allow to also ignore the setting of the local variable because we
        // assume the user entered the leading whitespace on purpose.
        if ignore.whitespace && command.chars().next().map_or(false, |b| b.is_whitespace()) {
            return false;
        }

        if ignore.no_such_command && self.previous_status == NO_SUCH_COMMAND {
            return false;
        }

        if ignore.duplicates {
            if let Some(ref mut context) = self.context {
                context.history.remove_duplicates(command);
                return true;
            } else {
                return false;
            }
        }

        if let Some(ref regexes) = self.ignore_setting.regexes.as_ref() {
            // ignore command when regex is matched but only if it does not contain
            // "HISTORY_IGNORE", otherwise we would also ignore the command which
            // sets the variable, which could be annoying.
            if regexes.iter().any(|regex| regex.is_match(command))
                && !command.contains("HISTORY_IGNORE")
            {
                return false;
            }
        }

        // default to true, as it's more likely that we want to save a command in
        // history
        true
    }
}
