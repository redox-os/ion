use std::io::{self, Write};
use std::iter;
use std::process::Command;

use directory_stack::DirectoryStack;
use glob::glob;
use parser::expand_string;
use parser::peg::RedirectFrom;
use parser::shell_expand::ExpandErr;
use variables::Variables;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum JobKind { And, Background, Last, Or, Pipe(RedirectFrom) }

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
    pub kind: JobKind,
}

impl Job {
    pub fn new(args: Vec<String>, kind: JobKind) -> Self {
        let command = args[0].clone();
        Job {
            command: command,
            args: args,
            kind: kind,
        }
    }

    pub fn expand_globs(&mut self) {
        let mut new_args: Vec<String> = vec![];
        for arg in self.args.drain(..) {
            let mut pushed_glob = false;
            if arg.contains(|chr| chr == '?' || chr == '*' || chr == '[') {
                if let Ok(expanded) = glob(&arg) {
                    for path in expanded.filter_map(Result::ok) {
                        pushed_glob = true;
                        new_args.push(path.to_string_lossy().into_owned());
                    }
                }
            }
            if !pushed_glob {
                new_args.push(arg);
            }
        }
        self.args = new_args;
    }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand(&mut self, variables: &Variables, dir_stack: &DirectoryStack) {
        // Expand each of the current job's arguments using the `expand_string` method.
        // If an error occurs, mark that error and break;
        let mut expanded: Vec<String> = Vec::new();
        let mut nth_argument = 0;
        let mut error_occurred = None;
        for (job, result) in self.args.iter().map(|argument| expand_string(argument, variables, dir_stack)).enumerate() {
            match result {
                Ok(expanded_string) => expanded.push(expanded_string),
                Err(cause) => {
                    nth_argument   = job;
                    error_occurred = Some(cause);
                    expanded = vec!["".to_owned()];
                    break
                }
            }
        }

        // If an error was detected, handle that error.
        if let Some(cause) = error_occurred {
            match cause {
                ExpandErr::UnmatchedBraces(position) => {
                    let original = self.args.join(" ");
                    let n_chars = self.args.iter().take(nth_argument)
                        .fold(0, |total, arg| total + 1 + arg.len()) + position;
                    let stderr = io::stderr();
                    let _ = writeln!(&mut stderr.lock(), "ion: expand error: unmatched braces\n{}\n{}^",
                        original, iter::repeat("-").take(n_chars).collect::<String>());
                },
                ExpandErr::InnerBracesNotImplemented => {
                    let stderr = io::stderr();
                    let _ = writeln!(&mut stderr.lock(), "ion: expand error: inner braces not yet implemented");
                }
            }
        }

        self.args = expanded;
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.command);
        for i in 1..self.args.len() {
            if let Some(arg) = self.args.get(i) {
                command.arg(arg);
            }
        }
        command
    }
}
