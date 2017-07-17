use std::process::Command;

//use glob::glob;
use parser::{expand_string, ExpanderFunctions};
use parser::peg::RedirectFrom;
use smallstring::SmallString;
use types::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum JobKind { And, Background, Last, Or, Pipe(RedirectFrom) }

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: Identifier,
    pub args: Array,
    pub kind: JobKind,
}

impl Job {
    pub fn new(args: Array, kind: JobKind) -> Self {
        let command = SmallString::from_str(&args[0]);
        Job {
            command: command,
            args: args,
            kind: kind,
        }
    }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand(&mut self, expanders: &ExpanderFunctions) {
        use smallvec::SmallVec;

        let mut expanded = SmallVec::new();
        expanded.grow(self.args.len());
        {
            for arg in self.args.drain().flat_map(|argument| expand_string(&argument, expanders, false)) {

                expanded.push(arg);
            }
        }

        self.args = expanded;
        self.command = self.args.first().map_or("".into(), |c| c.clone().into());
    }

    pub fn build_command_external(&mut self) -> Command {
        let mut command = Command::new(&self.command);
        for arg in self.args.drain().skip(1) {
            command.arg(arg);
        }
        command
    }

    pub fn build_command_builtin(&mut self) -> Command {
        use std::env;
        let process = env::current_exe().unwrap();
        let mut command = Command::new(process);
        command.arg("-c");
        command.arg(&self.command);
        for arg in self.args.drain().skip(1) {
            command.arg(arg);
        }
        command
    }
}
