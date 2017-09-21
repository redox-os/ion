mod collector;

pub(crate) use self::collector::*;

use super::{expand_string, Expander};
use shell::{Job, JobKind};
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
    pub jobs:   Vec<Job>,
    pub stdout: Option<Redirection>,
    pub stdin:  Option<Input>,
}

impl Pipeline {
    pub(crate) fn new(jobs: Vec<Job>, stdin: Option<Input>, stdout: Option<Redirection>) -> Self {
        Pipeline {
            jobs,
            stdin,
            stdout,
        }
    }

    pub(crate) fn expand<E: Expander>(&mut self, expanders: &E) {
        for job in &mut self.jobs {
            job.expand(expanders);
        }

        let stdin = match self.stdin {
            Some(Input::File(ref s)) => {
                Some(Input::File(expand_string(s, expanders, false).join(" ")))
            }
            Some(Input::HereString(ref s)) => {
                Some(Input::HereString(expand_string(s, expanders, true).join(" ")))
            }
            None => None,
        };

        self.stdin = stdin;

        if let Some(stdout) = self.stdout.iter_mut().next() {
            stdout.file = expand_string(stdout.file.as_str(), expanders, false).join(" ");
        }
    }

    pub(crate) fn requires_piping(&self) -> bool {
        self.jobs.len() > 1 || self.stdin != None || self.stdout != None ||
            self.jobs.last().unwrap().kind == JobKind::Background
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut tokens: Vec<String> = Vec::with_capacity(self.jobs.len());
        for job in &self.jobs {
            tokens.extend(job.args.clone().into_iter());
            match job.kind {
                JobKind::Last => (),
                JobKind::And => tokens.push("&&".into()),
                JobKind::Or => tokens.push("||".into()),
                JobKind::Background => tokens.push("&".into()),
                JobKind::Pipe(RedirectFrom::Stdout) => tokens.push("|".into()),
                JobKind::Pipe(RedirectFrom::Stderr) => tokens.push("^|".into()),
                JobKind::Pipe(RedirectFrom::Both) => tokens.push("&|".into()),
            }
        }
        match self.stdin {
            None => (),
            Some(Input::File(ref file)) => {
                tokens.push("<".into());
                tokens.push(file.clone());
            }
            Some(Input::HereString(ref string)) => {
                tokens.push("<<<".into());
                tokens.push(string.clone());
            }
        }
        if let Some(ref outfile) = self.stdout {
            match outfile.from {
                RedirectFrom::Stdout => {
                    tokens.push((if outfile.append { ">>" } else { ">" }).into());
                }
                RedirectFrom::Stderr => {
                    tokens.push((if outfile.append { "^>>" } else { "^>" }).into());
                }
                RedirectFrom::Both => {
                    tokens.push((if outfile.append { "&>>" } else { "&>" }).into());
                }
            }
            tokens.push(outfile.file.clone());
        }

        write!(f, "{}", tokens.join(" "))
    }
}
