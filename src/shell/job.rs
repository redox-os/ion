use std::process::Command;

use glob::glob;
use parser::{expand_string, ExpanderFunctions};
use parser::peg::RedirectFrom;

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

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand(&mut self, expanders: &ExpanderFunctions) {
        let mut expanded: Vec<String> = Vec::with_capacity(self.args.len());
        {
            let mut iterator = self.args.drain(..);
            expanded.push(iterator.next().unwrap());
            for arg in iterator.flat_map(|argument| expand_string(&argument, expanders, false)) {
                if arg.contains(|chr| chr == '?' || chr == '*' || chr == '[') {
                    if let Ok(glob) = glob(&arg) {
                        for path in glob.filter_map(Result::ok) {
                            expanded.push(path.to_string_lossy().into_owned());
                            continue
                        }
                    }
                }
                expanded.push(arg);
            }
        }

        self.args = expanded;
    }

    pub fn build_command(&mut self) -> Command {
        match CommandType::from(self.command.as_str()) {
            CommandType::Builtin => {
                use std::env;
                let process = env::current_exe().unwrap();
                let mut command = Command::new(process);
                command.arg("-c");
                command.arg(&self.command);
                for arg in self.args.drain(..).skip(1) {
                    command.arg(arg);
                }
                command
            },
            CommandType::External => {
                let mut command = Command::new(&self.command);
                for arg in self.args.drain(..).skip(1) {
                    command.arg(arg);
                }
                command
            }
        }
    }
}

enum CommandType {
    Builtin,
    External
}

impl<'a> From<&'a str> for CommandType {
    fn from(command: &'a str) -> CommandType {
        match command {
            "help" | "history" | "echo" => CommandType::Builtin,
            _ => CommandType::External
        }
    }
}
