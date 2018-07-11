//! Contains the binary logic of Ion.
mod designators;
mod prompt;
mod readln;
mod terminate;

use self::{
    prompt::{prompt, prompt_fn}, readln::readln,
    terminate::{terminate_quotes, terminate_script_quotes},
};
use super::{flow_control::Statement, status::*, FlowLogic, Shell, ShellHistory};
use types;
use liner::{Buffer, Context};
use std::{env, fs::File, io::ErrorKind, iter, path::Path, process, sync::Mutex};

pub const MAN_ION: &str = r#"NAME
    ion - ion shell

SYNOPSIS
    ion [ -h | --help ] [-c] [-n] [-v]

DESCRIPTION
    ion is a commandline shell created to be a faster and easier to use alternative to the
    currently available shells. It is not POSIX compliant.

OPTIONS
    -c
        evaluates given commands instead of reading from the commandline.

    -n or --no-execute
        do not execute any commands, just do syntax checking.

    -v or --version
        prints the version, platform and revision of ion then exits.
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
    fn display_version(&self) {
        println!("{}", include!(concat!(env!("OUT_DIR"), "/version_string")));
        process::exit(0);
    }

    fn save_command(&mut self, cmd: &str) {
        if cmd.starts_with('~') {
            if !cmd.ends_with('/')
                && self
                    .variables
                    .tilde_expansion(cmd, &self.directory_stack)
                    .map_or(false, |path| Path::new(&path).is_dir())
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

    fn reset_flow(&mut self) {
        self.flow_control.level = 0;
        self.flow_control.current_if_mode = 0;
        self.flow_control.current_statement = Statement::Default;
    }

    fn execute_interactive(mut self) {
        self.context = Some({
            let mut context = Context::new();
            context.word_divider_fn = Box::new(word_divide);
            if "1" == self.get_str_or_empty("HISTFILE_ENABLED") {
                let path = self.get::<types::Str>("HISTFILE").expect("shell didn't set HISTFILE");
                context.history.set_file_name(Some(path.to_string()));
                if !Path::new(path.as_str()).exists() {
                    eprintln!("ion: creating history file at \"{}\"", path);
                    if let Err(why) = File::create(&*path) {
                        eprintln!("ion: could not create history file: {}", why);
                    }
                }
                match context.history.load_history() {
                    Ok(()) => {
                        // pass
                    }
                    Err(ref err) if err.kind() == ErrorKind::NotFound => {
                        let history_filename = self.get_str_or_empty("HISTFILE");
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
            Mutex::new(context)
        });

        self.evaluate_init_file();

        self.variables
            .set("args", iter::once(env::args().next().unwrap().into()).collect::<types::Array>());

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

    fn terminate_quotes(&mut self, command: String) -> Result<String, ()> {
        terminate_quotes(self, command)
    }

    fn terminate_script_quotes<I: Iterator<Item = String>>(&mut self, lines: I) -> i32 {
        terminate_script_quotes(self, lines)
    }

    fn readln(&mut self) -> Option<String> { readln(self) }

    fn prompt_fn(&mut self) -> Option<String> { prompt_fn(self) }

    fn prompt(&mut self) -> String { prompt(self) }
}

#[derive(Debug)]
struct WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    iter:       I,
    count:      usize,
    word_start: Option<usize>,
}
impl<I> WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    #[inline]
    fn check_boundary(&mut self, c: char, index: usize, escaped: bool) -> Option<(usize, usize)> {
        if let Some(start) = self.word_start {
            if c == ' ' && !escaped {
                self.word_start = None;
                Some((start, index))
            } else {
                self.next()
            }
        } else {
            if c != ' ' {
                self.word_start = Some(index);
            }
            self.next()
        }
    }
}
impl<I> Iterator for WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;
        match self.iter.next() {
            Some((i, '\\')) => {
                if let Some((_, cnext)) = self.iter.next() {
                    self.count += 1;
                    // We use `i` in order to include the backslash as part of the word
                    self.check_boundary(cnext, i, true)
                } else {
                    self.next()
                }
            }
            Some((i, c)) => self.check_boundary(c, i, false),
            None => {
                // When start has been set, that means we have encountered a full word.
                self.word_start.take().map(|start| (start, self.count - 1))
            }
        }
    }
}

fn word_divide(buf: &Buffer) -> Vec<(usize, usize)> {
    // -> impl Iterator<Item = (usize, usize)> + 'a
    WordDivide {
        iter:       buf.chars().cloned().enumerate(),
        count:      0,
        word_start: None,
    }.collect() // TODO: return iterator directly :D
}
