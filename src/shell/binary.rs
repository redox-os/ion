//! Contains the binary logic of Ion.

use liner::{BasicCompleter, Buffer, Context, Event, EventKind, CursorPosition};
use parser::*;
use parser::QuoteTerminator;
use smallstring::SmallString;
use smallvec::SmallVec;
use std::env;
use std::fs::File;
use std::io::{self, Write, Read, ErrorKind};
use std::iter::{self, FromIterator};
use std::mem;
use std::path::{Path, PathBuf};
use sys;
use super::completer::*;
use super::flow_control::Statement;
use super::status::*;
use super::{Shell, FlowLogic, JobControl, ShellHistory, Variables, DirectoryStack};
use types::*;

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
    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    fn readln(&mut self) -> Option<String>;
    /// Generates the prompt that will be used by Liner.
    fn prompt(&self) -> String;
}

impl<'a> Binary for Shell<'a> {
    fn prompt(&self) -> String {
        if self.flow_control.level == 0 {
            let prompt_var = self.variables.get_var_or_empty("PROMPT");
            expand_string(&prompt_var, &get_expanders!(&self.variables, &self.directory_stack), false).join(" ")
        } else {
            "    ".repeat(self.flow_control.level as usize)
        }
    }

    fn readln(&mut self) -> Option<String> {
        {
            let vars_ptr = &self.variables as *const Variables;
            let dirs_ptr = &self.directory_stack as *const DirectoryStack;
            let funcs = &self.functions;
            let vars = &self.variables;
            let builtins = self.builtins;

            // Collects the current list of values from history for completion.
            let history = &self.context.as_ref().unwrap().history.buffers.iter()
                // Map each underlying `liner::Buffer` into a `String`.
                .map(|x| x.chars().cloned().collect())
                // Collect each result into a vector to avoid borrowing issues.
                .collect::<Vec<SmallString>>();

            loop {
                let prompt = self.prompt();
                let line = self.context.as_mut().unwrap().read_line(prompt, &mut move |Event { editor, kind }| {
                    if let EventKind::BeforeComplete = kind {
                        let (words, pos) = editor.get_words_and_cursor_position();

                        let filename = match pos {
                            CursorPosition::InWord(index) => index > 0,
                            CursorPosition::InSpace(Some(_), _) => true,
                            CursorPosition::InSpace(None, _) => false,
                            CursorPosition::OnWordLeftEdge(index) => index >= 1,
                            CursorPosition::OnWordRightEdge(index) => {
                                match (words.into_iter().nth(index), env::current_dir()) {
                                    (Some((start, end)), Ok(file)) => {
                                        let filename = editor.current_buffer().range(start, end);
                                        complete_as_file(file, filename, index)
                                    },
                                    _ => false,
                                }
                            }
                        };

                        if filename {
                            if let Ok(current_dir) = env::current_dir() {
                                if let Some(url) = current_dir.to_str() {
                                    let completer = IonFileCompleter::new(Some(url), dirs_ptr, vars_ptr);
                                    mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                                }
                            }
                        } else {
                            // Creates a list of definitions from the shell environment that will be used
                            // in the creation of a custom completer.
                            let words = builtins.iter()
                                // Add built-in commands to the completer's definitions.
                                .map(|(&s, _)| Identifier::from(s))
                                // Add the history list to the completer's definitions.
                                .chain(history.iter().cloned())
                                // Add the aliases to the completer's definitions.
                                .chain(vars.aliases.keys().cloned())
                                // Add the list of available functions to the completer's definitions.
                                .chain(funcs.keys().cloned())
                                // Add the list of available variables to the completer's definitions.
                                // TODO: We should make it free to do String->SmallString
                                //       and mostly free to go back (free if allocated)
                                .chain(vars.get_vars().into_iter().map(|s| ["$", &s].concat().into()))
                                .collect();

                            // Initialize a new completer from the definitions collected.
                            let custom_completer = BasicCompleter::new(words);

                            // Creates completers containing definitions from all directories listed
                            // in the environment's **$PATH** variable.
                            let mut file_completers = if let Ok(val) = env::var("PATH") {
                                val.split(sys::PATH_SEPARATOR)
                                    .map(|s| IonFileCompleter::new(Some(s), dirs_ptr, vars_ptr))
                                    .collect()
                            } else {
                                vec![IonFileCompleter::new(Some("/bin/"), dirs_ptr, vars_ptr)]
                            };

                            // Also add files/directories in the current directory to the completion list.
                            if let Ok(current_dir) = env::current_dir() {
                                if let Some(url) = current_dir.to_str() {
                                    file_completers.push(IonFileCompleter::new(Some(url), dirs_ptr, vars_ptr));
                                }
                            }

                            // Merge the collected definitions with the file path definitions.
                            let completer = MultiCompleter::new(file_completers, custom_completer);

                            // Replace the shell's current completer with the newly-created completer.
                            mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                        }
                    }
                });

                match line {
                    Ok(line) => return Some(line),
                    // Handles Ctrl + C
                    Err(ref err) if err.kind() == ErrorKind::Interrupted => return None,
                    // Handles Ctrl + D
                    Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => break,
                    Err(err) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: liner: {}", err);
                        return None
                    }
                }
            }
        }

        let previous_status = self.previous_status;
        self.exit(previous_status);
    }

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

        self.evaluate_init_file();

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

/// Infer if the given filename is actually a partial filename
fn complete_as_file(current_dir : PathBuf, filename : String, index : usize) -> bool {
    let filename = filename.trim();
    let mut file = current_dir.clone();
    file.push(&filename);
    // If the user explicitly requests a file through this syntax then complete as a file
    if filename.trim().starts_with(".") { return true; }
    // If the file starts with a dollar sign, it's a variable, not a file
    if filename.trim().starts_with("$") { return false; }
    // Once we are beyond the first string, assume its a file
    if index > 0 { return true; }
    // If we are referencing a file that exists then just complete to that file
    if file.exists() { return true; }
    // If we have a partial file inside an existing directory, e.g. /foo/b when /foo/bar
    // exists, then treat it as file as long as `foo` isn't the current directory, otherwise
    // this would apply to any string `foo`
    if let Some(parent) = file.parent() { return parent.exists() && parent != current_dir; }
    // By default assume its not a file
    false
}
