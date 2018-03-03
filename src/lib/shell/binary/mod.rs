//! Contains the binary logic of Ion.
mod designators;
mod prompt;
mod readln;
mod terminate;

use self::prompt::{prompt, prompt_fn};
use self::readln::readln;
use self::terminate::{terminate_quotes, terminate_script_quotes};
use super::{FlowLogic, Shell, ShellHistory};
use super::flow_control::Statement;
use super::status::*;
use liner::{Buffer, Context};
use std::env;
use std::fs::File;
use std::io::ErrorKind;
use std::iter;
use std::path::Path;
use std::process;

pub const MAN_ION: &'static str = r#"NAME
    ion - ion shell

SYNOPSIS
    ion [ -h | --help ] [-c] [-n] [-v] [-l]

DESCRIPTION
    ion is a commandline shell created to be a faster and easier to use alternative to the 
    currently available shells. It is not POSIX compliant. 

OPTIONS
    -c
        evaulates given commands instead of reading from the commandline.

    -n or --no-execute
        do not execute any commands, just do syntax checking.

    -v or --version
        prints the version, platform and revision of ion then exits.

    -l or --login
        currently does nothing, however in the futere will run ion as a login shell.
"#;

pub trait Binary {
    /// Parses and executes the arguments that were supplied to the shell.
    fn execute_arguments<A: Iterator<Item = String>>(&mut self, args: A);
    /// Creates an interactive session that reads from a prompt provided by
    /// Liner.
    fn execute_interactive(self);
    /// Ensures that read statements from a script are terminated.
    fn terminate_script_quotes<I: Iterator<Item = String>>(&mut self, lines: I) -> i32;
    /// Ensures that read statements from the interactive prompt is terminated.
    fn terminate_quotes(&mut self, command: String) -> Result<String, ()>;
    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    fn readln(&mut self) -> Option<String>;
    /// Generates the prompt that will be used by Liner.
    fn prompt(&mut self) -> String;
    /// Display version information and exit
    fn display_version(&self);
    // Executes the PROMPT function, if it exists, and returns the output.
    fn prompt_fn(&mut self) -> Option<String>;
    // Handles commands given by the REPL, and saves them to history.
    fn save_command(&mut self, command: &str);
    // Resets the flow control fields to their default values.
    fn reset_flow(&mut self);
}

impl Binary for Shell {
    fn prompt(&mut self) -> String { prompt(self) }

    fn prompt_fn(&mut self) -> Option<String> { prompt_fn(self) }

    fn readln(&mut self) -> Option<String> { readln(self) }

    fn terminate_script_quotes<I: Iterator<Item = String>>(&mut self, lines: I) -> i32 {
        terminate_script_quotes(self, lines)
    }

    fn terminate_quotes(&mut self, command: String) -> Result<String, ()> {
        terminate_quotes(self, command)
    }

    fn execute_arguments<A: Iterator<Item = String>>(&mut self, mut args: A) {
        if let Some(mut arg) = args.next() {
            for argument in args {
                arg.push(' ');
                if argument == "" {
                    arg.push_str("''");
                } else {
                    arg.push_str(&argument);
                }
            }
            self.on_command(&arg);
        } else {
            eprintln!("ion: -c requires an argument");
            self.exit(FAILURE);
        }

        if self.flow_control.level != 0 {
            eprintln!(
                "ion: unexpected end of arguments: expected end block for `{}`",
                self.flow_control.current_statement.short()
            );
            self.exit(FAILURE);
        }
    }

    fn execute_interactive(mut self) {
        self.context = Some({
            let mut context = Context::new();
            context.word_divider_fn = Box::new(word_divide);
            if "1" == self.get_var_or_empty("HISTFILE_ENABLED") {
                let path = self.get_var("HISTFILE").expect("shell didn't set HISTFILE");
                context.history.set_file_name(Some(path.clone()));
                if !Path::new(path.as_str()).exists() {
                    eprintln!("ion: creating history file at \"{}\"", path);
                    if let Err(why) = File::create(path) {
                        eprintln!("ion: could not create history file: {}", why);
                    }
                }
                match context.history.load_history() {
                    Ok(()) => {
                        // pass
                    }
                    Err(ref err) if err.kind() == ErrorKind::NotFound => {
                        let history_filename = self.get_var_or_empty("HISTFILE");
                        eprintln!(
                            "ion: failed to find history file {}: {}",
                            history_filename, err
                        );
                    }
                    Err(err) => {
                        eprintln!("ion: failed to load history: {}", err);
                    }
                }
            }
            context
        });

        self.evaluate_init_file();

        self.variables
            .set_array("args", iter::once(env::args().next().unwrap()).collect());

        loop {
            if let Some(command) = self.readln() {
                if !command.is_empty() {
                    if let Ok(command) = self.terminate_quotes(command.replace("\\\n", "")) {
                        let cmd: &str = &designators::expand_designators(&self, command.trim());
                        self.on_command(&cmd);
                        self.save_command(&cmd);
                    } else {
                        self.reset_flow();
                    }
                }
            } else {
                self.reset_flow();
            }
        }
    }

    fn reset_flow(&mut self) {
        self.flow_control.level = 0;
        self.flow_control.current_if_mode = 0;
        self.flow_control.current_statement = Statement::Default;
    }

    fn save_command(&mut self, cmd: &str) {
        if cmd.starts_with('~') {
            if !cmd.ends_with('/')
                && self.variables
                    .tilde_expansion(cmd, &self.directory_stack)
                    .map_or(false, |ref path| Path::new(path).is_dir())
            {
                self.save_command_in_history(&[cmd, "/"].concat());
            } else {
                self.save_command_in_history(cmd);
            }
            return;
        }

        if Path::new(cmd).is_dir() & !cmd.ends_with('/') {
            self.save_command_in_history(&[cmd, "/"].concat());
        } else {
            self.save_command_in_history(cmd);
        }
    }

    fn display_version(&self) {
        println!("{}", include!(concat!(env!("OUT_DIR"), "/version_string")));
        process::exit(0);
    }
}

// TODO: Convert this into an iterator to eliminate heap allocations.
fn word_divide(buf: &Buffer) -> Vec<(usize, usize)> {
    let mut res = Vec::new();
    let mut word_start = None;

    macro_rules! check_boundary {
        ($c:expr, $index:expr, $escaped:expr) => {{
            if let Some(start) = word_start {
                if $c == ' ' && !$escaped {
                    res.push((start, $index));
                    word_start = None;
                }
            } else {
                if $c != ' ' {
                    word_start = Some($index);
                }
            }
        }}
    }

    let mut iter = buf.chars().enumerate();
    while let Some((i, &c)) = iter.next() {
        match c {
            '\\' => {
                if let Some((_, &cnext)) = iter.next() {
                    // We use `i` in order to include the backslash as part of the word
                    check_boundary!(cnext, i, true);
                }
            }
            c => check_boundary!(c, i, false),
        }
    }
    if let Some(start) = word_start {
        // When start has been set, that means we have encountered a full word.
        res.push((start, buf.num_chars()));
    }
    res
}
