use super::{completer::IonCompleter, InteractiveShell};
use ion_shell::{builtins::Status, Shell, Value};
use regex::Regex;
use rustyline::Editor;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct IgnoreSetting {
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
    regexes: Vec<Regex>,
}

/// Contains all history-related functionality for the `Shell`.
impl InteractiveShell {
    /// Updates the history ignore patterns. Call this whenever HISTORY_IGNORE
    /// is changed.
    pub fn ignore_patterns(&self, shell: &Shell<'_>) -> IgnoreSetting {
        if let Some(Value::Array(patterns)) = shell.variables().get("HISTORY_IGNORE") {
            let mut settings = IgnoreSetting::default();
            // for convenience and to avoid typos
            let regex_prefix = "regex:";
            for pattern in patterns.iter() {
                let pattern = format!("{}", pattern);
                match pattern.as_ref() {
                    "all" => settings.all = true,
                    "no_such_command" => settings.no_such_command = true,
                    "whitespace" => settings.whitespace = true,
                    "duplicates" => settings.duplicates = true,
                    // The length check is there to just ignore empty regex definitions
                    _ if pattern.starts_with(regex_prefix)
                        && pattern.len() > regex_prefix.len() =>
                    {
                        settings.based_on_regex = true;
                        let regex_string = &pattern[regex_prefix.len()..];
                        // We save the compiled regexes, as compiling them can be  an expensive task
                        if let Ok(regex) = Regex::new(regex_string) {
                            settings.regexes.push(regex);
                        }
                    }
                    _ => continue,
                }
            }

            settings
        } else {
            panic!("HISTORY_IGNORE is not set!");
        }
    }

    /// Saves a command in the history, depending on @HISTORY_IGNORE. Should be called
    /// immediately after `on_command()`
    pub fn save_command_in_history(
        &self,
        command: &str,
        shell: &mut Shell<'_>,
        context: &mut Editor<IonCompleter<'_, '_>>,
    ) {
        if self.should_save_command(command, shell) {
            if shell.variables().get_str("HISTORY_TIMESTAMP").unwrap_or_default() == "1" {
                // Get current time stamp
                let since_unix_epoch =
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let cur_time_sys = format!("#{}", &since_unix_epoch);

                // Push current time to history
                context.history_mut().add(cur_time_sys);
            }

            // Push command itself to history
            context.history_mut().add(command);
            let helper = context.helper_mut().unwrap();
            if helper.should_save() {
                if let Some(Value::Str(histfile)) = shell.variables().get("HISTFILE") {
                    let histfile = histfile.clone();
                    std::mem::drop(shell);
                    let helper = context.helper_mut().unwrap();
                    if helper.load_on_flush() {
                        if let Err(err) = context.history_mut().load(&histfile.as_str()) {
                            eprintln!("ion: could not save history to file: {}", err);
                        }
                    }
                    if let Err(err) = context.history().save(&histfile.as_str()) {
                        eprintln!("ion: could not save history to file: {}", err);
                    }
                }
            }
        }
    }

    /// Returns true if the given command with the given exit status should be saved in the
    /// history
    fn should_save_command(&self, command: &str, shell: &Shell<'_>) -> bool {
        // just for convenience and to make the code look a bit cleaner
        let ignore = self.ignore_patterns(shell);

        // without the second check the command which sets the local variable would
        // also be ignored. However, this behavior might not be wanted.
        if ignore.all && !command.contains("HISTORY_IGNORE") {
            return false;
        }

        // Here we allow to also ignore the setting of the local variable because we
        // assume the user entered the leading whitespace on purpose.
        if ignore.whitespace && command.chars().next().map_or(false, char::is_whitespace) {
            return false;
        }

        if ignore.no_such_command && shell.previous_status() == Status::NO_SUCH_COMMAND {
            return false;
        }

        // ignore command when regex is matched but only if it does not contain
        // "HISTORY_IGNORE", otherwise we would also ignore the command which
        // sets the variable, which could be annoying.
        if !command.contains("HISTORY_IGNORE")
            && ignore.regexes.iter().any(|regex| regex.is_match(command))
        {
            return false;
        }

        // default to true, as it's more likely that we want to save a command in
        // history
        true
    }
}
