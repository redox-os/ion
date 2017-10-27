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

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum RedirectKind {
    None,
    Single(Redirection),
    Multiple(Vec<Redirection>),
}

impl RedirectKind {
    /// Analogue to `Option::take` for `RedirectKind`
    pub(crate) fn take(&mut self) -> RedirectKind {
        ::std::mem::replace(self, RedirectKind::None)
    }
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
    pub output: RedirectKind,
    pub stdin:  Option<Input>,
}

impl Pipeline {
    pub(crate) fn new(jobs: Vec<Job>,
                      stdin: Option<Input>,
                      output: RedirectKind) -> Self {
        Pipeline {
            jobs,
            stdin,
            output,
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

        match self.output {
            RedirectKind::None => {},
            RedirectKind::Single(ref mut out) =>
                out.file = expand_string(out.file.as_str(), expanders, false).join(" "),
            RedirectKind::Multiple(ref mut outs) =>
                outs.iter_mut().for_each(|out| {
                    out.file = expand_string(out.file.as_str(), expanders, false).join(" ");
                }),
        }
    }

    pub(crate) fn requires_piping(&self) -> bool {
        self.jobs.len() > 1 || self.stdin != None || self.output != RedirectKind::None
            || self.jobs.last().unwrap().kind == JobKind::Background
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
        let append_redirect = |tokens: &mut Vec<String>, redirect: &Redirection| {
            match redirect.from {
                RedirectFrom::Stdout => {
                    tokens.push((if redirect.append { ">>" } else { ">" }).into());
                }
                RedirectFrom::Stderr => {
                    tokens.push((if redirect.append { "^>>" } else { "^>" }).into());
                }
                RedirectFrom::Both => {
                    tokens.push((if redirect.append { "&>>" } else { "&>" }).into());
                }
            }
            tokens.push(redirect.file.clone());
        };
        match self.output {
            RedirectKind::None => {},
            RedirectKind::Single(ref out) => append_redirect(&mut tokens, out),
            RedirectKind::Multiple(ref outs) => outs.iter().for_each(|out| append_redirect(&mut tokens, out)),
        }
        write!(f, "{}", tokens.join(" "))
    }
}
