mod collector;

pub use self::collector::*;

use crate::{
    parser::Expander,
    shell::{pipe_exec::stdin_of, Job, Shell},
    types,
};
use itertools::Itertools;
use small;
use std::{fmt, fs::File, os::unix::io::FromRawFd};

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
    pub file:   small::String,
    pub append: bool,
}

/// Represents input that a process could initially receive from `stdin`
#[derive(Debug, PartialEq, Clone)]
pub enum Input {
    /// A file; the contents of said file will be written to the `stdin` of a
    /// process
    File(small::String),
    /// A string literal that is written to the `stdin` of a process.
    /// If there is a second string, that second string is the EOF phrase for the heredoc.
    HereString(small::String),
}

impl Input {
    pub fn get_infile(&mut self) -> Result<File, ()> {
        match self {
            Input::File(ref filename) => match File::open(filename.as_str()) {
                Ok(file) => Ok(file),
                Err(e) => {
                    eprintln!("ion: failed to redirect '{}' to stdin: {}", filename, e);
                    Err(())
                }
            },
            Input::HereString(ref mut string) => {
                if !string.ends_with('\n') {
                    string.push('\n');
                }
                match unsafe { stdin_of(&string) } {
                    Ok(stdio) => Ok(unsafe { File::from_raw_fd(stdio) }),
                    Err(e) => {
                        eprintln!(
                            "ion: failed to redirect herestring '{}' to stdin: {}",
                            string, e
                        );
                        Err(())
                    }
                }
            }
        }
    }
}

impl<'a> fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Input::File(ref file) => write!(f, "< {}", file),
            Input::HereString(ref string) => write!(f, "<<< '{}'", string),
        }
    }
}

impl<'a> fmt::Display for Redirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Debug, PartialEq, Clone)]
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
    pub fn expand(&mut self, shell: &Shell<'a>) {
        self.job.expand(shell);

        for input in &mut self.inputs {
            *input = match input {
                Input::File(ref s) => Input::File(shell.get_string(s)),
                Input::HereString(ref s) => Input::HereString(shell.get_string(s)),
            };
        }

        for output in &mut self.outputs {
            output.file = shell.get_string(output.file.as_str());
        }
    }

    pub fn command(&self) -> &types::Str { self.job.command() }

    pub fn new(job: Job<'a>, outputs: Vec<Redirection>, inputs: Vec<Input>) -> Self {
        PipeItem { job, outputs, inputs }
    }
}

impl<'a> fmt::Display for PipeItem<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

    pub fn expand(&mut self, shell: &Shell<'a>) {
        self.items.iter_mut().for_each(|i| i.expand(shell));
    }

    pub fn new() -> Self { Pipeline { items: Vec::new(), pipe: PipeType::Normal } }
}

impl<'a> fmt::Display for Pipeline<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
