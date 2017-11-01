//! Contains the binary logic of Ion.
mod prompt;
mod readln;
mod terminate;

use self::prompt::{prompt, prompt_fn};
use self::readln::readln;
use self::terminate::{terminate_quotes, terminate_script_quotes};
use super::{FlowLogic, JobControl, Shell, ShellHistory};
use super::flags::*;
use super::flow_control::Statement;
use super::library::IonLibrary;
use super::status::*;
use liner::{Buffer, Context};
use smallvec::SmallVec;
use std::env;
use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::iter::{self, FromIterator};
use std::path::Path;
use std::process;

pub(crate) trait Binary {
    /// Launches the shell, parses arguments, and then diverges into one of the `execution`
    /// paths.
    fn main(self);
    /// Parses and executes the arguments that were supplied to the shell.
    fn execute_arguments<A: Iterator<Item = String>>(&mut self, args: A);
    /// Creates an interactive session that reads from a prompt provided by Liner.
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
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: -c requires an argument");
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
            if "1" == self.variables.get_var_or_empty("HISTFILE_ENABLED") {
                let path = self.variables.get_var("HISTFILE").expect("shell didn't set HISTFILE");
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
                        let history_filename = self.variables.get_var_or_empty("HISTFILE");
                        eprintln!("ion: failed to find history file {}: {}", history_filename, err);
                    }
                    Err(err) => {
                        eprintln!("ion: failed to load history: {}", err);
                    }
                }
            }
            context
        });

        self.evaluate_init_file();

        self.variables.set_array("args", iter::once(env::args().next().unwrap()).collect());

        loop {
            if let Some(command) = self.readln() {
                if !command.is_empty() {
                    if let Ok(command) = self.terminate_quotes(command.replace("\\\n", "")) {
                        let cmd = command.trim();
                        self.on_command(cmd);

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
                            self.update_variables();
                            continue;
                        }

                        if Path::new(cmd).is_dir() & !cmd.ends_with('/') {
                            self.save_command_in_history(&[cmd, "/"].concat());
                        } else {
                            self.save_command_in_history(cmd);
                        }
                    } else {
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                        self.flow_control.current_statement = Statement::Default;
                    }
                }
                self.update_variables();
            } else {
                self.flow_control.level = 0;
                self.flow_control.current_if_mode = 0;
                self.flow_control.current_statement = Statement::Default;
            }
        }
    }

    fn main(mut self) {
        let mut args = env::args().skip(1);
        while let Some(path) = args.next() {
            match path.as_str() {
                "-n" => {
                    self.flags |= NO_EXEC;
                    continue;
                }
                "-c" => self.execute_arguments(args),
                "--version" => self.display_version(),
                _ => {
                    let mut array = SmallVec::from_iter(Some(path.clone().into()));
                    for arg in args {
                        array.push(arg.into());
                    }
                    self.variables.set_array("args", array);
                    if let Err(err) = self.execute_script(&path) {
                        eprintln!("ion: {}", err);
                    }
                }
            }

            self.wait_for_background();
            let previous_status = self.previous_status;
            self.exit(previous_status);
        }

        self.execute_interactive();
    }

    fn display_version(&self) {
        println!("{}", include!(concat!(env!("OUT_DIR"), "/version_string")));
        process::exit(0);
    }
}

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
