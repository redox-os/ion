mod collector;

pub(crate) use self::collector::*;

use super::expand_string;
use shell::{Job, JobKind, Shell};
use std::fmt;

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum RedirectFrom {
    Stdout,
    Stderr,
    Both,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Redirection {
    pub from:   RedirectFrom,
    pub file:   String,
    pub append: bool,
}


/// Represents input that a process could initially receive from `stdin`
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Input {
    /// A file; the contents of said file will be written to the `stdin` of a process
    File(String),
    /// A string literal that is written to the `stdin` of a process.
    /// If there is a second string, that second string is the EOF phrase for the heredoc.
    HereString(String),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Pipeline {
    pub items: Vec<PipeItem>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct PipeItem {
    pub job:     Job,
    pub outputs: Vec<Redirection>,
    pub inputs:  Vec<Input>,
}

impl PipeItem {
    pub(crate) fn new(job: Job, outputs: Vec<Redirection>, inputs: Vec<Input>) -> Self {
        PipeItem { job, outputs, inputs }
    }

    pub(crate) fn expand(&mut self, shell: &Shell) {
        self.job.expand(shell);

        for input in self.inputs.iter_mut() {
            *input = match input {
                &mut Input::File(ref s) => Input::File(expand_string(s, shell, false).join(" ")),
                &mut Input::HereString(ref s) => {
                    Input::HereString(expand_string(s, shell, true).join(" "))
                }
            };
        }

        for output in self.outputs.iter_mut() {
            output.file = expand_string(output.file.as_str(), shell, false).join(" ");
        }
    }
}

impl Pipeline {
    pub(crate) fn new() -> Self { Pipeline { items: Vec::new() } }

    pub(crate) fn expand(&mut self, shell: &Shell) {
        self.items.iter_mut().for_each(|i| i.expand(shell));
    }

    pub(crate) fn requires_piping(&self) -> bool {
        self.items.len() > 1 || self.items.iter().any(|it| it.outputs.len() > 0)
            || self.items.iter().any(|it| it.inputs.len() > 0)
            || self.items.last().unwrap().job.kind == JobKind::Background
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut tokens: Vec<String> = Vec::with_capacity(self.items.len());
        for item in &self.items {
            let job = &item.job;
            let kind = job.kind;
            let inputs = &item.inputs;
            let outputs = &item.outputs;
            tokens.extend(item.job.args.clone().into_iter());
            for input in inputs {
                match input {
                    &Input::File(ref file) => {
                        tokens.push("<".into());
                        tokens.push(file.clone());
                    }
                    &Input::HereString(ref string) => {
                        tokens.push("<<<".into());
                        tokens.push(string.clone());
                    }
                }
            }
            for output in outputs {
                match output.from {
                    RedirectFrom::Stdout => {
                        tokens.push((if output.append { ">>" } else { ">" }).into());
                    }
                    RedirectFrom::Stderr => {
                        tokens.push((if output.append { "^>>" } else { "^>" }).into());
                    }
                    RedirectFrom::Both => {
                        tokens.push((if output.append { "&>>" } else { "&>" }).into());
                    }
                }
                tokens.push(output.file.clone());
            }
            match kind {
                JobKind::Last => (),
                JobKind::And => tokens.push("&&".into()),
                JobKind::Or => tokens.push("||".into()),
                JobKind::Background => tokens.push("&".into()),
                JobKind::Pipe(RedirectFrom::Stdout) => tokens.push("|".into()),
                JobKind::Pipe(RedirectFrom::Stderr) => tokens.push("^|".into()),
                JobKind::Pipe(RedirectFrom::Both) => tokens.push("&|".into()),
            }
        }

        write!(f, "{}", tokens.join(" "))
    }
}
