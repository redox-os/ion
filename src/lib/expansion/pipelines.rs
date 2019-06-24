use super::Expander;
use crate::{
    shell::{Job, Shell},
    types,
};
use itertools::Itertools;
use std::fmt;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RedirectFrom {
    Stdout,
    Stderr,
    Both,
    None,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Redirection {
    pub from:   RedirectFrom,
    pub file:   types::Str,
    pub append: bool,
}

/// Represents input that a process could initially receive from `stdin`
#[derive(Debug, PartialEq, Clone)]
pub enum Input {
    /// A file; the contents of said file will be written to the `stdin` of a
    /// process
    File(types::Str),
    /// A string literal that is written to the `stdin` of a process.
    /// If there is a second string, that second string is the EOF phrase for the heredoc.
    HereString(types::Str),
}

impl<'a> fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Input::File(ref file) => write!(f, "< {}", file),
            Input::HereString(ref string) => write!(f, "<<< '{}'", string),
        }
    }
}

impl<'a> fmt::Display for Redirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}>{} {}",
            match self.from {
                RedirectFrom::Stdout => "",
                RedirectFrom::Stderr => "^",
                RedirectFrom::Both => "&",
                RedirectFrom::None => unreachable!(),
            },
            if self.append { ">" } else { "" },
            self.file,
        )
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PipeType {
    Normal,
    Background,
    Disown,
}

impl Default for PipeType {
    fn default() -> Self { PipeType::Normal }
}

#[derive(Default, Debug, PartialEq, Clone)]
pub struct Pipeline<'a> {
    pub items: Vec<PipeItem<'a>>,
    pub pipe:  PipeType,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PipeItem<'a> {
    pub job:     Job<'a>,
    pub outputs: Vec<Redirection>,
    pub inputs:  Vec<Input>,
}

impl<'a> PipeItem<'a> {
    pub fn expand(&self, shell: &Shell<'a>) -> super::Result<Self, <Shell as Expander>::Error> {
        let mut job = self.job.clone();
        job.expand(shell)?;

        let inputs = self
            .inputs
            .iter()
            .map(|input| match input {
                Input::File(ref s) => shell.get_string(s).map(Input::File),
                Input::HereString(ref s) => shell.get_string(s).map(Input::HereString),
            })
            .collect::<Result<_, _>>()?;

        let outputs = self
            .outputs
            .iter()
            .map(|output| {
                shell.get_string(output.file.as_str()).map(|file| {
                    let mut output = output.clone();
                    output.file = file;
                    output
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(PipeItem { job, outputs, inputs })
    }

    pub fn command(&self) -> &types::Str { self.job.command() }

    pub fn new(job: Job<'a>, outputs: Vec<Redirection>, inputs: Vec<Input>) -> Self {
        PipeItem { job, outputs, inputs }
    }
}

impl<'a> fmt::Display for PipeItem<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.job.args.iter().format(" "))?;
        for input in &self.inputs {
            write!(f, " {}", input)?;
        }
        for output in &self.outputs {
            write!(f, " {}", output)?;
        }
        write!(
            f,
            "{}",
            match self.job.redirection {
                RedirectFrom::None => "",
                RedirectFrom::Stdout => " |",
                RedirectFrom::Stderr => " ^|",
                RedirectFrom::Both => " &|",
            }
        )
    }
}

impl<'a> Pipeline<'a> {
    pub fn requires_piping(&self) -> bool {
        self.items.len() > 1
            || self.items.iter().any(|it| !it.outputs.is_empty())
            || self.items.iter().any(|it| !it.inputs.is_empty())
            || self.pipe != PipeType::Normal
    }

    pub fn expand(&self, shell: &Shell<'a>) -> super::Result<Self, <Shell as Expander>::Error> {
        let items = self.items.iter().map(|i| i.expand(shell)).collect::<Result<_, _>>()?;
        Ok(Pipeline { items, pipe: self.pipe })
    }

    pub fn new() -> Self { Self::default() }
}

impl<'a> fmt::Display for Pipeline<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            self.items.iter().format(" "),
            match self.pipe {
                PipeType::Normal => "",
                PipeType::Background => " &",
                PipeType::Disown => " &!",
            }
        )
    }
}
