//! Contains the binary logic of Ion.

use liner::{Buffer, Context};
use smallvec::SmallVec;
use std::env;
use std::fs::File;
use std::io::{self, Write, Read, ErrorKind};
use std::iter::{self, FromIterator};
use std::path::Path;
use super::flow_control::Statement;
use super::status::*;
use super::{Shell, FlowLogic, JobControl, ShellHistory};
use parser::QuoteTerminator;

pub trait Binary {
    /// Launches the shell, parses arguments, and then diverges into one of the `execution` paths.
    fn main(self);
    /// Parses and executes the arguments that were supplied to the shell.
    fn execute_arguments<A: Iterator<Item = String>>(&mut self, args: A);
    /// Creates an interactive session that reads from a prompt provided by Liner.
    fn execute_interactive(self);
    /// Executes all of the statements contained within a given script.
    fn execute_script<P: AsRef<Path>>(&mut self, path: P);
    /// Ensures that read statements from a script are terminated.
    fn terminate_script_quotes<I: Iterator<Item = String>>(&mut self, lines: I);
    /// Ensures that read statements from the interactive prompt is terminated.
    fn terminate_quotes(&mut self, command: String) -> Result<String, ()>;
}

impl<'a> Binary for Shell<'a> {
    fn terminate_script_quotes<I: Iterator<Item = String>>(&mut self, mut lines: I) {
        while let Some(command) = lines.next() {
            let mut buffer = QuoteTerminator::new(command);
            while !buffer.check_termination() {
                loop {
                    if let Some(command) = lines.next() {
                        buffer.append(command);
                        break
                    } else {
                        let stderr = io::stderr();
                        let _ = writeln!(stderr.lock(), "ion: unterminated quote in script");
                        self.exit(FAILURE);
                    }
                }
            }
            self.on_command(&buffer.consume());
        }
        // The flow control level being non zero means that we have a statement that has
        // only been partially parsed.
        if self.flow_control.level != 0 {
            eprintln!("ion: unexpected end of script: expected end block for `{}`",
                self.flow_control.current_statement.short());
        }
    }

    fn terminate_quotes(&mut self, command: String) -> Result<String, ()> {
        let mut buffer = QuoteTerminator::new(command);
        self.flow_control.level += 1;
        while !buffer.check_termination() {
            loop {
                if let Some(command) = self.readln() {
                    buffer.append(command);
                    break
                } else {
                    return Err(());
                }
            }
        }
        self.flow_control.level -= 1;
        Ok(buffer.consume())
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
    }

    fn execute_interactive(mut self) {
        self.context = Some({
            let mut context = Context::new();
            context.word_divider_fn = Box::new(word_divide);
            if "1" == self.variables.get_var_or_empty("HISTORY_FILE_ENABLED") {
                let path = self.variables.get_var("HISTORY_FILE").expect("shell didn't set history_file");
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
                        let history_filename = self.variables.get_var_or_empty("HISTORY_FILE");
                        eprintln!("ion: failed to find history file {}: {}", history_filename, err);
                    },
                    Err(err) => {
                        eprintln!("ion: failed to load history: {}", err);
                    }
                }
            }
            context
        });

        self.variables.set_array (
            "args",
            iter::once(env::args().next().unwrap()).collect(),
        );

        loop {
            if let Some(command) = self.readln() {
                if ! command.is_empty() {
                    if let Ok(command) = self.terminate_quotes(command) {
                        // Parse and potentially execute the command.
                        self.on_command(command.trim());

                        // Mark the command in the context history if it was a success.
                        if self.previous_status != NO_SUCH_COMMAND || self.flow_control.level > 0 {
                            self.set_context_history_from_vars();
                            if let Err(err) = self.context.as_mut().unwrap().history.push(command.into()) {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "ion: {}", err);
                            }
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
        if let Some(path) = args.next() {
            if path == "-c" {
                self.execute_arguments(args);
            } else {
                let mut array = SmallVec::from_iter(
                    Some(path.clone().into())
                );
                for arg in args { array.push(arg.into()); }
                self.variables.set_array("args", array);
                self.execute_script(&path);
            }

            self.wait_for_background();
            let previous_status = self.previous_status;
            self.exit(previous_status);
        } else {
            self.execute_interactive();
        }
    }

    fn execute_script<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        match File::open(path) {
            Ok(mut file) => {
                let capacity = file.metadata().ok().map_or(0, |x| x.len());
                let mut command_list = String::with_capacity(capacity as usize);
                match file.read_to_string(&mut command_list) {
                    Ok(_) => self.terminate_script_quotes(command_list.lines().map(|x| x.to_owned())),
                    Err(err) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: failed to read {:?}: {}", path, err);
                    }
                }
            },
            Err(err) => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: failed to open {:?}: {}", path, err);
            }
        }
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
