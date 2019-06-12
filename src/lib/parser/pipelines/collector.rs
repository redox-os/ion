use err_derive::Error;
use std::iter::Peekable;

use super::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection};
use crate::{
    builtins::BuiltinMap,
    lexers::arguments::{Field, Levels, LevelsError},
    shell::Job,
    types::*,
};

const ARG_DEFAULT_SIZE: usize = 10;

#[derive(Debug, Error)]
pub enum PipelineParsingError {
    // redirections
    #[error(display = "expected file argument after redirection for output")]
    NoRedirection,
    #[error(display = "heredocs are not a part of Ion. Use redirection and/or cat instead")]
    HeredocsDeprecated,
    #[error(display = "expected string argument after '<<<'")]
    NoHereStringArg,
    #[error(display = "expected file argument after redirection for input")]
    NoRedirectionArg,

    // quotes
    #[error(display = "unterminated double quote")]
    UnterminatedDoubleQuote,
    #[error(display = "unterminated single quote")]
    UnterminatedSingleQuote,

    // paired
    #[error(display = "{}", _0)]
    Paired(#[error(cause)] LevelsError),
}

impl From<LevelsError> for PipelineParsingError {
    fn from(cause: LevelsError) -> Self { PipelineParsingError::Paired(cause) }
}

trait AddItem<'a> {
    fn add_item(
        &mut self,
        redirection: RedirectFrom,
        args: Args,
        outputs: Vec<Redirection>,
        inputs: Vec<Input>,
        builtin: &BuiltinMap<'a>,
    );
}

impl<'a> AddItem<'a> for Pipeline<'a> {
    fn add_item(
        &mut self,
        redirection: RedirectFrom,
        args: Args,
        outputs: Vec<Redirection>,
        inputs: Vec<Input>,
        builtins: &BuiltinMap<'a>,
    ) {
        if !args.is_empty() {
            let builtin = builtins.get(&args[0]);
            self.items.push(PipeItem::new(Job::new(args, redirection, builtin), outputs, inputs));
        }
    }
}

#[derive(Debug)]
pub struct Collector<'a> {
    data: &'a str,
}

impl<'a> Collector<'a> {
    /// Add a new argument that is re
    fn push_arg<I>(
        &self,
        args: &mut Args,
        bytes: &mut Peekable<I>,
    ) -> Result<(), PipelineParsingError>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        if let Some(v) = self.arg(bytes)? {
            args.push(v.into());
        }
        Ok(())
    }

    /// Attempt to add a redirection
    fn push_redir_to_output<I>(
        &self,
        from: RedirectFrom,
        outputs: &mut Vec<Redirection>,
        bytes: &mut Peekable<I>,
    ) -> Result<(), PipelineParsingError>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        let append = if let Some(&(_, b'>')) = bytes.peek() {
            bytes.next();
            true
        } else {
            false
        };
        self.arg(bytes)?
            .ok_or(PipelineParsingError::NoRedirection)
            .map(|file| outputs.push(Redirection { from, file: file.into(), append }))
    }

    pub fn parse<'builtins>(
        &self,
        builtins: &BuiltinMap<'builtins>,
    ) -> Result<Pipeline<'builtins>, PipelineParsingError> {
        let mut bytes = self.data.bytes().enumerate().peekable();
        let mut args = Args::with_capacity(ARG_DEFAULT_SIZE);
        let mut pipeline = Pipeline::new();
        let mut outputs: Vec<Redirection> = Vec::new();
        let mut inputs: Vec<Input> = Vec::new();

        while let Some(&(i, b)) = bytes.peek() {
            // Determine what production rule we are using based on the first character
            match b {
                b'&' => {
                    // We have effectively consumed this byte
                    bytes.next();
                    match bytes.peek() {
                        Some(&(_, b'>')) => {
                            // And this byte
                            bytes.next();
                            self.push_redir_to_output(
                                RedirectFrom::Both,
                                &mut outputs,
                                &mut bytes,
                            )?;
                        }
                        Some(&(_, b'|')) => {
                            bytes.next();
                            pipeline.add_item(
                                RedirectFrom::Both,
                                std::mem::replace(&mut args, Args::with_capacity(ARG_DEFAULT_SIZE)),
                                std::mem::replace(&mut outputs, Vec::new()),
                                std::mem::replace(&mut inputs, Vec::new()),
                                builtins,
                            );
                        }
                        Some(&(_, b'!')) => {
                            bytes.next();
                            pipeline.pipe = PipeType::Disown;
                            break;
                        }
                        Some(_) | None => {
                            pipeline.pipe = PipeType::Background;
                            break;
                        }
                    }
                }
                b'^' => {
                    // We do not immediately consume this byte as it could just be the start of
                    // a new argument
                    match self.peek(i + 1) {
                        Some(b'>') => {
                            bytes.next();
                            bytes.next();
                            self.push_redir_to_output(
                                RedirectFrom::Stderr,
                                &mut outputs,
                                &mut bytes,
                            )?;
                        }
                        Some(b'|') => {
                            bytes.next();
                            bytes.next();
                            pipeline.add_item(
                                RedirectFrom::Stderr,
                                std::mem::replace(&mut args, Args::with_capacity(ARG_DEFAULT_SIZE)),
                                std::mem::replace(&mut outputs, Vec::new()),
                                std::mem::replace(&mut inputs, Vec::new()),
                                builtins,
                            );
                        }
                        Some(_) | None => self.push_arg(&mut args, &mut bytes)?,
                    }
                }
                b'|' => {
                    bytes.next();
                    pipeline.add_item(
                        RedirectFrom::Stdout,
                        std::mem::replace(&mut args, Args::with_capacity(ARG_DEFAULT_SIZE)),
                        std::mem::replace(&mut outputs, Vec::new()),
                        std::mem::replace(&mut inputs, Vec::new()),
                        builtins,
                    );
                }
                b'>' => {
                    bytes.next();
                    self.push_redir_to_output(RedirectFrom::Stdout, &mut outputs, &mut bytes)?;
                }
                b'<' => {
                    bytes.next();
                    if Some(b'<') == self.peek(i + 1) {
                        if Some(b'<') == self.peek(i + 2) {
                            // If the next two characters are arrows, then interpret
                            // the next argument as a herestring
                            bytes.next();
                            bytes.next();
                            if let Some(cmd) = self.arg(&mut bytes)? {
                                inputs.push(Input::HereString(cmd.into()));
                            } else {
                                return Err(PipelineParsingError::NoHereStringArg);
                            }
                        } else {
                            return Err(PipelineParsingError::HeredocsDeprecated);
                        }
                    } else if let Some(file) = self.arg(&mut bytes)? {
                        // Otherwise interpret it as stdin redirection
                        inputs.push(Input::File(file.into()));
                    } else {
                        return Err(PipelineParsingError::NoRedirectionArg);
                    }
                }
                // Skip over whitespace between jobs
                b' ' | b'\t' => {
                    bytes.next();
                }
                // Assume that the next character starts an argument and parse that argument
                _ => self.push_arg(&mut args, &mut bytes)?,
            }
        }

        pipeline.add_item(RedirectFrom::None, args, outputs, inputs, builtins);
        Ok(pipeline)
    }

    fn arg<I>(&self, bytes: &mut Peekable<I>) -> Result<Option<&'a str>, PipelineParsingError>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        // XXX: I don't think its the responsibility of the pipeline parser to do this
        // but I'm not sure of a better solution
        let mut levels = Levels::default();
        let mut start = None;
        let mut end = None;
        // Array increments * 2 + 1; brace * 2
        // Supports up to 31 nested arrays
        let mut array_brace_counter: u32 = 0;

        // Skip over any leading whitespace
        while let Some(&(_, b)) = bytes.peek() {
            match b {
                b' ' | b'\t' => {
                    bytes.next();
                }
                _ => break,
            }
        }

        while let Some(&(i, b)) = bytes.peek() {
            if start.is_none() {
                start = Some(i)
            }
            match b {
                b'(' => {
                    levels.up(Field::Proc);
                    bytes.next();
                }
                b')' => {
                    levels.down(Field::Proc);
                    bytes.next();
                }
                b'[' => {
                    levels.up(Field::Array);
                    array_brace_counter = array_brace_counter.wrapping_mul(2) + 1;
                    bytes.next();
                }
                b']' => {
                    levels.down(Field::Array);
                    if array_brace_counter % 2 == 1 {
                        array_brace_counter = (array_brace_counter - 1) / 2;
                        bytes.next();
                    } else {
                        break;
                    }
                }
                b'{' => {
                    levels.up(Field::Braces);
                    array_brace_counter = array_brace_counter.wrapping_mul(2);
                    bytes.next();
                }
                b'}' => {
                    if array_brace_counter % 2 == 0 {
                        levels.down(Field::Braces);
                        array_brace_counter /= 2;
                        bytes.next();
                    } else {
                        break;
                    }
                }
                // This is a tricky one: we only end the argment if `^` is followed by a
                // redirection character
                b'^' => {
                    if levels.are_rooted() {
                        if let Some(next_byte) = self.peek(i + 1) {
                            // If the next byte is for stderr to file or next process, end this
                            // argument
                            if next_byte == b'>' || next_byte == b'|' {
                                end = Some(i);
                                break;
                            }
                        }
                    }
                    // Reaching this block means that either there is no next byte, or the next
                    // byte is none of '>' or '|', indicating that this is not the beginning of
                    // a redirection for stderr
                    bytes.next();
                }
                // Evaluate a quoted string but do not return it
                // We pass in i, the index of a quote, but start a character later. This ensures
                // the production rules will produce strings with the quotes intact
                b'"' => {
                    bytes.next();
                    self.double_quoted(bytes, i)?;
                }
                b'\'' => {
                    bytes.next();
                    self.single_quoted(bytes, i)?;
                }
                // If we see a backslash, assume that it is leading up to an escaped character
                // and skip the next character
                b'\\' => {
                    bytes.next();
                    bytes.next();
                }
                // If we see a byte from the follow set, we've definitely reached the end of
                // the arguments
                b'&' | b'|' | b'<' | b'>' | b' ' | b'\t' if levels.are_rooted() => {
                    end = Some(i);
                    break;
                }
                // By default just pop the next byte: it will be part of the argument
                _ => {
                    bytes.next();
                }
            }
        }

        levels.check()?;

        match (start, end) {
            (Some(i), Some(j)) if i < j => Ok(Some(&self.data[i..j])),
            (Some(i), None) => Ok(Some(&self.data[i..])),
            _ => Ok(None),
        }
    }

    fn double_quoted<I>(
        &self,
        bytes: &mut Peekable<I>,
        start: usize,
    ) -> Result<&'a str, PipelineParsingError>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        while let Some(&(i, b)) = bytes.peek() {
            match b {
                b'\\' => {
                    bytes.next();
                }
                // We return an inclusive range to keep the quote type intact
                b'"' => {
                    bytes.next();
                    return Ok(&self.data[start..=i]);
                }
                _ => (),
            }
            bytes.next();
        }
        Err(PipelineParsingError::UnterminatedDoubleQuote)
    }

    fn single_quoted<I>(
        &self,
        bytes: &mut Peekable<I>,
        start: usize,
    ) -> Result<&'a str, PipelineParsingError>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        while let Some(&(i, b)) = bytes.peek() {
            // We return an inclusive range to keep the quote type intact
            if b == b'\'' {
                bytes.next();
                return Ok(&self.data[start..=i]);
            }
            bytes.next();
        }
        Err(PipelineParsingError::UnterminatedSingleQuote)
    }

    fn peek(&self, index: usize) -> Option<u8> {
        if index < self.data.len() {
            Some(self.data.as_bytes()[index])
        } else {
            None
        }
    }

    pub fn run<'builtins>(
        data: &'a str,
        builtins: &BuiltinMap<'builtins>,
    ) -> Result<Pipeline<'builtins>, PipelineParsingError> {
        Collector::new(data).parse(builtins)
    }

    pub fn new(data: &'a str) -> Self { Collector { data } }
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins::BuiltinMap,
        parser::{
            pipelines::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection},
            statement::parse,
        },
        shell::{flow_control::Statement, Job},
    };

    #[test]
    fn stderr_redirection() {
        if let Statement::Pipeline(pipeline) =
            parse("git rev-parse --abbrev-ref HEAD ^> /dev/null", &BuiltinMap::new()).unwrap()
        {
            assert_eq!("git", &pipeline.items[0].job.args[0]);
            assert_eq!("rev-parse", &pipeline.items[0].job.args[1]);
            assert_eq!("--abbrev-ref", &pipeline.items[0].job.args[2]);
            assert_eq!("HEAD", &pipeline.items[0].job.args[3]);

            let expected = vec![Redirection {
                from:   RedirectFrom::Stderr,
                file:   "/dev/null".into(),
                append: false,
            }];

            assert_eq!(expected, pipeline.items[0].outputs);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn braces() {
        if let Statement::Pipeline(pipeline) =
            parse("echo {a b} {a {b c}}", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("{a b}", &items[0].job.args[1]);
            assert_eq!("{a {b c}}", &items[0].job.args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn methods() {
        if let Statement::Pipeline(pipeline) =
            parse("echo @split(var, ', ') $join(array, ',')", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("@split(var, ', ')", &items[0].job.args[1]);
            assert_eq!("$join(array, ',')", &items[0].job.args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_process() {
        if let Statement::Pipeline(pipeline) =
            parse("echo $(echo one $(echo two) three)", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("$(echo one $(echo two) three)", &items[0].job.args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_array_process() {
        if let Statement::Pipeline(pipeline) =
            parse("echo @(echo one @(echo two) three)", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("@(echo one @(echo two) three)", &items[0].job.args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn quoted_process() {
        if let Statement::Pipeline(pipeline) =
            parse("echo \"$(seq 1 10)\"", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("\"$(seq 1 10)\"", &items[0].job.args[1]);
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn process() {
        if let Statement::Pipeline(pipeline) =
            parse("echo $(seq 1 10 | head -1)", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("$(seq 1 10 | head -1)", &items[0].job.args[1]);
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn array_process() {
        if let Statement::Pipeline(pipeline) =
            parse("echo @(seq 1 10 | head -1)", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("@(seq 1 10 | head -1)", &items[0].job.args[1]);
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_no_args() {
        if let Statement::Pipeline(pipeline) = parse("cat", &BuiltinMap::new()).unwrap() {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("cat", items[0].command());
            assert_eq!(1, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_with_single_character_arguments() {
        if let Statement::Pipeline(pipeline) = parse("echo a b c", &BuiltinMap::new()).unwrap() {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("echo", &items[0].job.args[0]);
            assert_eq!("a", &items[0].job.args[1]);
            assert_eq!("b", &items[0].job.args[2]);
            assert_eq!("c", &items[0].job.args[3]);
            assert_eq!(4, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn job_with_args() {
        if let Statement::Pipeline(pipeline) = parse("ls -al dir", &BuiltinMap::new()).unwrap() {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("ls", &items[0].job.args[0]);
            assert_eq!("-al", &items[0].job.args[1]);
            assert_eq!("dir", &items[0].job.args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn parse_empty_string() {
        if let Statement::Default = parse("", &BuiltinMap::new()).unwrap() {
            return;
        } else {
            assert!(false);
        }
    }

    #[test]
    fn multiple_white_space_between_words() {
        if let Statement::Pipeline(pipeline) =
            parse("ls \t -al\t\tdir", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("ls", &items[0].job.args[0]);
            assert_eq!("-al", &items[0].job.args[1]);
            assert_eq!("dir", &items[0].job.args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn trailing_whitespace() {
        if let Statement::Pipeline(pipeline) = parse("ls -al\t ", &BuiltinMap::new()).unwrap() {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("ls", &pipeline.items[0].job.args[0]);
            assert_eq!("-al", &pipeline.items[0].job.args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn double_quoting() {
        if let Statement::Pipeline(pipeline) =
            parse("echo \"a > 10\" \"a < 10\"", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("\"a > 10\"", &items[0].job.args[1]);
            assert_eq!("\"a < 10\"", &items[0].job.args[2]);
            assert_eq!(3, items[0].job.args.len());
        } else {
            assert!(false)
        }
    }

    #[test]
    fn double_quoting_contains_single() {
        if let Statement::Pipeline(pipeline) =
            parse("echo \"Hello 'Rusty' World\"", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\"Hello \'Rusty\' World\"", &items[0].job.args[1]);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn multi_quotes() {
        if let Statement::Pipeline(pipeline) =
            parse("echo \"Hello \"Rusty\" World\"", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\"Hello \"Rusty\" World\"", &items[0].job.args[1]);
        } else {
            assert!(false)
        }

        if let Statement::Pipeline(pipeline) =
            parse("echo \'Hello \'Rusty\' World\'", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\'Hello \'Rusty\' World\'", &items[0].job.args[1]);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn all_whitespace() {
        if let Statement::Default = parse("  \t ", &BuiltinMap::new()).unwrap() {
            return;
        } else {
            assert!(false);
        }
    }

    #[test]
    fn not_background_job() {
        if let Statement::Pipeline(pipeline) =
            parse("echo hello world", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!(RedirectFrom::None, items[0].job.redirection);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job() {
        if let Statement::Pipeline(pipeline) =
            parse("echo hello world&", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(PipeType::Background, pipeline.pipe);
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) =
            parse("echo hello world &", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(PipeType::Background, pipeline.pipe);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn disown_job() {
        if let Statement::Pipeline(pipeline) =
            parse("echo hello world&!", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(PipeType::Disown, pipeline.pipe);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn lone_comment() {
        if let Statement::Default = parse("# ; \t as!!+dfa", &BuiltinMap::new()).unwrap() {
            return;
        } else {
            assert!(false);
        }
    }

    #[test]
    fn leading_whitespace() {
        if let Statement::Pipeline(pipeline) = parse("    \techo", &BuiltinMap::new()).unwrap() {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("echo", items[0].command());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_quoting() {
        if let Statement::Pipeline(pipeline) = parse("echo '#!!;\"\\'", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("'#!!;\"\\'", &items[0].job.args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        if let Statement::Pipeline(pipeline) =
            parse("echo 123 456 \"ABC 'DEF' GHI\" 789 one'  'two", &BuiltinMap::new()).unwrap()
        {
            let items = pipeline.items;
            assert_eq!("123", &items[0].job.args[1]);
            assert_eq!("456", &items[0].job.args[2]);
            assert_eq!("\"ABC 'DEF' GHI\"", &items[0].job.args[3]);
            assert_eq!("789", &items[0].job.args[4]);
            assert_eq!("one'  'two", &items[0].job.args[5]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn several_blank_lines() {
        if let Statement::Default = parse("\n\n\n", &BuiltinMap::new()).unwrap() {
            return;
        } else {
            assert!(false);
        }
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection() {
        let input = "cat | echo hello | cat < stuff > other";
        if let Statement::Pipeline(pipeline) = parse(input, &BuiltinMap::new()).unwrap() {
            assert_eq!(3, pipeline.items.len());
            assert_eq!("cat", &pipeline.items[0].job.args[0]);
            assert_eq!("echo", &pipeline.items[1].job.args[0]);
            assert_eq!("hello", &pipeline.items[1].job.args[1]);
            assert_eq!("cat", &pipeline.items[2].job.args[0]);
            assert_eq!(vec![Input::File("stuff".into())], pipeline.items[2].inputs);
            assert_eq!("other", &pipeline.items[2].outputs[0].file);
            assert!(!pipeline.items[2].outputs[0].append);
            assert_eq!(input.to_owned(), pipeline.to_string());
        } else {
            assert!(false);
        }
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append() {
        if let Statement::Pipeline(pipeline) =
            parse("cat | echo hello | cat < stuff >> other", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(3, pipeline.items.len());
            assert_eq!(Input::File("stuff".into()), pipeline.items[2].inputs[0]);
            assert_eq!("other", &pipeline.items[2].outputs[0].file);
            assert!(pipeline.items[2].outputs[0].append);
        } else {
            assert!(false);
        }
    }

    #[test]
    // Ensures no regression for infinite loop when args() hits
    // '^' while not in the top level
    fn args_loop_terminates() {
        if let Statement::Pipeline(pipeline) = parse("$(^) '$(^)'", &BuiltinMap::new()).unwrap() {
            assert_eq!("$(^)", &pipeline.items[0].job.args[0]);
            assert_eq!("\'$(^)\'", &pipeline.items[0].job.args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn multiple_redirect() {
        let input = "cat < file1 <<< \"herestring\" | tr 'x' 'y' ^>> err &> both > out";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job:     Job::new(args!["cat"], RedirectFrom::Stdout, None),
                    inputs:  vec![
                        Input::File("file1".into()),
                        Input::HereString("\"herestring\"".into()),
                    ],
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(args!["tr", "'x'", "'y'"], RedirectFrom::None, None),
                    inputs:  Vec::new(),
                    outputs: vec![
                        Redirection {
                            from:   RedirectFrom::Stderr,
                            file:   "err".into(),
                            append: true,
                        },
                        Redirection {
                            from:   RedirectFrom::Both,
                            file:   "both".into(),
                            append: false,
                        },
                        Redirection {
                            from:   RedirectFrom::Stdout,
                            file:   "out".into(),
                            append: false,
                        },
                    ],
                },
            ],
            pipe:  PipeType::Normal,
        };
        assert_eq!(parse(input, &BuiltinMap::new()).unwrap(), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append_stderr() {
        let input = "cat | echo hello | cat < stuff ^>> other";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job: Job::new(args!["cat"], RedirectFrom::Stdout, None),

                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job: Job::new(args!["echo", "hello"], RedirectFrom::Stdout, None),

                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job: Job::new(args!["cat"], RedirectFrom::None, None),

                    inputs:  vec![Input::File("stuff".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Stderr,
                        file:   "other".into(),
                        append: true,
                    }],
                },
            ],
            pipe:  PipeType::Normal,
        };
        assert_eq!(parse(input, &BuiltinMap::new()).unwrap(), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append_both() {
        let input = "cat | echo hello | cat < stuff &>> other";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job: Job::new(args!["cat"], RedirectFrom::Stdout, None),

                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job: Job::new(args!["echo", "hello"], RedirectFrom::Stdout, None),

                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job: Job::new(args!["cat"], RedirectFrom::None, None),

                    inputs:  vec![Input::File("stuff".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Both,
                        file:   "other".into(),
                        append: true,
                    }],
                },
            ],
            pipe:  PipeType::Normal,
        };
        assert_eq!(parse(input, &BuiltinMap::new()).unwrap(), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_reverse_order() {
        if let Statement::Pipeline(pipeline) =
            parse("cat | echo hello | cat > stuff < other", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(3, pipeline.items.len());
            assert_eq!(vec![Input::File("other".into())], pipeline.items[2].inputs);
            assert_eq!("stuff", &pipeline.items[2].outputs[0].file);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn var_meets_quote() {
        if let Statement::Pipeline(pipeline) =
            parse("echo $x '{()}' test", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("echo", &pipeline.items[0].job.args[0]);
            assert_eq!("$x", &pipeline.items[0].job.args[1]);
            assert_eq!("'{()}'", &pipeline.items[0].job.args[2]);
            assert_eq!("test", &pipeline.items[0].job.args[3]);
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) =
            parse("echo $x'{()}' test", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("echo", &pipeline.items[0].job.args[0]);
            assert_eq!("$x'{()}'", &pipeline.items[0].job.args[1]);
            assert_eq!("test", &pipeline.items[0].job.args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn herestring() {
        let input = "calc <<< $(cat math.txt)";
        let expected = Pipeline {
            items: vec![PipeItem {
                job: Job::new(args!["calc"], RedirectFrom::None, None),

                inputs:  vec![Input::HereString("$(cat math.txt)".into())],
                outputs: vec![],
            }],
            pipe:  PipeType::Normal,
        };
        assert_eq!(Statement::Pipeline(expected), parse(input, &BuiltinMap::new()).unwrap());
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn piped_herestring() {
        let input = "cat | tr 'o' 'x' <<< $VAR > out.log";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job: Job::new(args!["cat"], RedirectFrom::Stdout, None),

                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job: Job::new(args!["tr", "'o'", "'x'"], RedirectFrom::None, None),

                    inputs:  vec![Input::HereString("$VAR".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Stdout,
                        file:   "out.log".into(),
                        append: false,
                    }],
                },
            ],
            pipe:  PipeType::Normal,
        };
        assert_eq!(Statement::Pipeline(expected), parse(input, &BuiltinMap::new()).unwrap());
    }

    #[test]
    fn awk_tests() {
        if let Statement::Pipeline(pipeline) =
            parse("awk -v x=$x '{ if (1) print $1 }' myfile", &BuiltinMap::new()).unwrap()
        {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("awk", &pipeline.items[0].job.args[0]);
            assert_eq!("-v", &pipeline.items[0].job.args[1]);
            assert_eq!("x=$x", &pipeline.items[0].job.args[2]);
            assert_eq!("'{ if (1) print $1 }'", &pipeline.items[0].job.args[3]);
            assert_eq!("myfile", &pipeline.items[0].job.args[4]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn escaped_filenames() {
        let input = "echo zardoz >> foo\\'bar";
        let expected = Pipeline {
            items: vec![PipeItem {
                job: Job::new(args!["echo", "zardoz"], RedirectFrom::None, None),

                inputs:  Vec::new(),
                outputs: vec![Redirection {
                    from:   RedirectFrom::Stdout,
                    file:   "foo\\'bar".into(),
                    append: true,
                }],
            }],
            pipe:  PipeType::Normal,
        };
        assert_eq!(parse(input, &BuiltinMap::new()).unwrap(), Statement::Pipeline(expected));
    }

    fn assert_parse_error(s: &str) {
        assert!(super::Collector::new(s).parse(&BuiltinMap::new()).is_err());
    }

    #[test]
    fn arrays_braces_out_of_order() {
        assert_parse_error("echo {[}]");
        assert_parse_error("echo [{]}");
    }

    #[test]
    fn unmatched_right_brackets() {
        assert_parse_error("]");
        assert_parse_error("}");
        assert_parse_error(")");
    }

}
