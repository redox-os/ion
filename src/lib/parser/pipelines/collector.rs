#![allow(eq_op)] // Required as a macro sets this clippy warning off.

use std::{collections::HashSet, iter::Peekable};

use super::{Input, PipeItem, Pipeline, RedirectFrom, Redirection};
use shell::{Job, JobKind};
use types::*;

#[derive(Debug)]
pub(crate) struct Collector<'a> {
    data: &'a str,
}

lazy_static! {
    /// The set of bytes that will always indicate an end of an arg
    static ref FOLLOW_ARGS: HashSet<u8> = b"&|<> \t".into_iter().cloned().collect();
}

impl<'a> Collector<'a> {
    pub(crate) fn parse(&self) -> Result<Pipeline, &'static str> {
        let mut bytes = self.data.bytes().enumerate().peekable();
        let mut args = Array::new();
        let mut pipeline = Pipeline::new();
        let mut outputs: Option<Vec<Redirection>> = None;
        let mut inputs: Option<Vec<Input>> = None;

        /// Add a new argument that is re
        macro_rules! push_arg {
            () => {{
                if let Some(v) = self.arg(&mut bytes)? {
                    args.push(v.into());
                }
            }};
        }

        /// Attempt to add a redirection
        macro_rules! try_redir_out {
            ($from:expr) => {{
                if outputs.is_none() {
                    outputs = Some(Vec::new());
                }
                let append = if let Some(&(_, b'>')) = bytes.peek() {
                    bytes.next();
                    true
                } else {
                    false
                };
                if let Some(file) = self.arg(&mut bytes)? {
                    outputs.as_mut().map(|o| {
                        o.push(Redirection {
                            from: $from,
                            file: file.into(),
                            append,
                        })
                    });
                } else {
                    return Err("expected file argument after redirection for output");
                }
            }};
        };

        /// Attempt to create a pipeitem and append it to the pipeline
        macro_rules! try_add_item {
            ($job_kind:expr) => {{
                if !args.is_empty() {
                    let job = Job::new(args.clone(), $job_kind);
                    args.clear();
                    let item_out = if let Some(out_tmp) = outputs.take() {
                        out_tmp
                    } else {
                        Vec::new()
                    };
                    let item_in = if let Some(in_tmp) = inputs.take() {
                        in_tmp
                    } else {
                        Vec::new()
                    };
                    pipeline.items.push(PipeItem::new(job, item_out, item_in));
                }
            }};
        }

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
                            try_redir_out!(RedirectFrom::Both);
                        }
                        Some(&(_, b'|')) => {
                            bytes.next();
                            try_add_item!(JobKind::Pipe(RedirectFrom::Both));
                        }
                        Some(&(_, b'!')) => {
                            bytes.next();
                            try_add_item!(JobKind::Disown);
                        }
                        Some(_) | None => {
                            try_add_item!(JobKind::Background);
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
                            try_redir_out!(RedirectFrom::Stderr);
                        }
                        Some(b'|') => {
                            bytes.next();
                            bytes.next();
                            try_add_item!(JobKind::Pipe(RedirectFrom::Stderr));
                        }
                        Some(_) | None => push_arg!(),
                    }
                }
                b'|' => {
                    bytes.next();
                    try_add_item!(JobKind::Pipe(RedirectFrom::Stdout));
                }
                b'>' => {
                    bytes.next();
                    try_redir_out!(RedirectFrom::Stdout);
                }
                b'<' => {
                    if inputs.is_none() {
                        inputs = Some(Vec::new());
                    }
                    bytes.next();
                    if Some(b'<') == self.peek(i + 1) {
                        if Some(b'<') == self.peek(i + 2) {
                            // If the next two characters are arrows, then interpret
                            // the next argument as a herestring
                            bytes.next();
                            bytes.next();
                            if let Some(cmd) = self.arg(&mut bytes)? {
                                if let Some(x) = inputs.as_mut() {
                                    x.push(Input::HereString(cmd.into()))
                                };
                            } else {
                                return Err("expected string argument after '<<<'");
                            }
                        } else {
                            // Otherwise, what we have is not a herestring, but a heredoc.
                            bytes.next();
                            // Collect the rest of the byte iterator and then trim the result
                            // in order to get the EOF phrase that will be used to terminate
                            // the heredoc.
                            let heredoc = {
                                let mut buffer = Vec::new();
                                while let Some((_, byte)) = bytes.next() {
                                    buffer.push(byte);
                                }
                                unsafe { String::from_utf8_unchecked(buffer) }
                            };
                            let heredoc = heredoc.lines().skip(1).collect::<Vec<&str>>();
                            if heredoc.len() > 1 {
                                let herestring = Input::HereString(
                                    heredoc[..heredoc.len() - 1].join("\n").into(),
                                );
                                if let Some(x) = inputs.as_mut() {
                                    x.push(herestring.clone())
                                };
                            }
                        }
                    } else if let Some(file) = self.arg(&mut bytes)? {
                        // Otherwise interpret it as stdin redirection
                        if let Some(x) = inputs.as_mut() {
                            x.push(Input::File(file.into()))
                        };
                    } else {
                        return Err("expected file argument after redirection for input");
                    }
                }
                // Skip over whitespace between jobs
                b' ' | b'\t' => {
                    bytes.next();
                }
                // Assume that the next character starts an argument and parse that argument
                _ => push_arg!(),
            }
        }

        if !args.is_empty() {
            try_add_item!(JobKind::Last);
        }

        Ok(pipeline)
    }

    fn arg<I>(&self, bytes: &mut Peekable<I>) -> Result<Option<&'a str>, &'static str>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        // XXX: I don't think its the responsibility of the pipeline parser to do this
        // but I'm not sure of a better solution
        let mut array_level = 0;
        let mut proc_level = 0;
        let mut brace_level = 0;
        let mut start = None;
        let mut end = None;
        // Array increments * 2 + 1; brace * 2
        // Supports up to 31 nested arrays
        let mut array_brace_counter: u32 = 0;

        macro_rules! is_toplevel {
            () => {
                array_level + proc_level + brace_level == 0
            };
        }

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
                    proc_level += 1;
                    bytes.next();
                }
                b')' => {
                    proc_level -= 1;
                    bytes.next();
                }
                b'[' => {
                    array_level += 1;
                    array_brace_counter = array_brace_counter.wrapping_mul(2) + 1;
                    bytes.next();
                }
                b']' => {
                    array_level -= 1;
                    if array_brace_counter % 2 == 1 {
                        array_brace_counter = (array_brace_counter - 1) / 2;
                        bytes.next();
                    } else {
                        break;
                    }
                }
                b'{' => {
                    brace_level += 1;
                    array_brace_counter = array_brace_counter.wrapping_mul(2);
                    bytes.next();
                }
                b'}' => if array_brace_counter % 2 == 0 {
                    brace_level -= 1;
                    array_brace_counter /= 2;
                    bytes.next();
                } else {
                    break;
                },
                // This is a tricky one: we only end the argment if `^` is followed by a
                // redirection character
                b'^' => {
                    if is_toplevel!() {
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
                c if FOLLOW_ARGS.contains(&c) && is_toplevel!() => {
                    end = Some(i);
                    break;
                }
                // By default just pop the next byte: it will be part of the argument
                _ => {
                    bytes.next();
                }
            }
        }
        if proc_level > 0 {
            return Err("ion: syntax error: unmatched left paren");
        }
        if array_level > 0 {
            return Err("ion: syntax error: unmatched left bracket");
        }
        if brace_level > 0 {
            return Err("ion: syntax error: unmatched left brace");
        }
        if proc_level < 0 {
            return Err("ion: syntax error: extra right paren(s)");
        }
        if array_level < 0 {
            return Err("ion: syntax error: extra right bracket(s)");
        }
        if brace_level < 0 {
            return Err("ion: syntax error: extra right brace(s)");
        }
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
    ) -> Result<&'a str, &'static str>
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
                    return Ok(&self.data[start..i + 1]);
                }
                _ => (),
            }
            bytes.next();
        }
        Err("ion: syntax error: unterminated quote")
    }

    fn single_quoted<I>(
        &self,
        bytes: &mut Peekable<I>,
        start: usize,
    ) -> Result<&'a str, &'static str>
    where
        I: Iterator<Item = (usize, u8)>,
    {
        while let Some(&(i, b)) = bytes.peek() {
            // We return an inclusive range to keep the quote type intact
            if let b'\'' = b {
                bytes.next();
                return Ok(&self.data[start..i + 1]);
            }
            bytes.next();
        }
        Err("ion: syntax error: unterminated single quote")
    }

    fn peek(&self, index: usize) -> Option<u8> {
        if index < self.data.len() {
            Some(self.data.as_bytes()[index])
        } else {
            None
        }
    }

    pub(crate) fn run(data: &'a str) -> Result<Pipeline, &'static str> {
        Collector::new(data).parse()
    }

    pub(crate) fn new(data: &'a str) -> Self { Collector { data } }
}

#[cfg(test)]
mod tests {
    use parser::{
        pipelines::{Input, PipeItem, Pipeline, RedirectFrom, Redirection},
        statement::parse,
    };
    use shell::{flow_control::Statement, Job, JobKind};
    use types::Array;

    #[test]
    fn stderr_redirection() {
        if let Statement::Pipeline(pipeline) = parse("git rev-parse --abbrev-ref HEAD ^> /dev/null")
        {
            assert_eq!("git", pipeline.items[0].job.args[0].as_str());
            assert_eq!("rev-parse", pipeline.items[0].job.args[1].as_str());
            assert_eq!("--abbrev-ref", pipeline.items[0].job.args[2].as_str());
            assert_eq!("HEAD", pipeline.items[0].job.args[3].as_str());

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
        if let Statement::Pipeline(pipeline) = parse("echo {a b} {a {b c}}") {
            let items = pipeline.items;
            assert_eq!("{a b}", items[0].job.args[1].as_str());
            assert_eq!("{a {b c}}", items[0].job.args[2].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn methods() {
        if let Statement::Pipeline(pipeline) = parse("echo @split(var, ', ') $join(array, ',')") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!("@split(var, ', ')", items[0].job.args[1].as_str());
            assert_eq!("$join(array, ',')", items[0].job.args[2].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_process() {
        if let Statement::Pipeline(pipeline) = parse("echo $(echo one $(echo two) three)") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!(
                "$(echo one $(echo two) three)",
                items[0].job.args[1].as_str()
            );
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_array_process() {
        if let Statement::Pipeline(pipeline) = parse("echo @(echo one @(echo two) three)") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!(
                "@(echo one @(echo two) three)",
                items[0].job.args[1].as_str()
            );
        } else {
            assert!(false);
        }
    }

    #[test]
    fn quoted_process() {
        if let Statement::Pipeline(pipeline) = parse("echo \"$(seq 1 10)\"") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!("\"$(seq 1 10)\"", items[0].job.args[1].as_str());
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn process() {
        if let Statement::Pipeline(pipeline) = parse("echo $(seq 1 10 | head -1)") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!("$(seq 1 10 | head -1)", items[0].job.args[1].as_str());
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn array_process() {
        if let Statement::Pipeline(pipeline) = parse("echo @(seq 1 10 | head -1)") {
            let items = pipeline.items;
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!("@(seq 1 10 | head -1)", items[0].job.args[1].as_str());
            assert_eq!(2, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_no_args() {
        if let Statement::Pipeline(pipeline) = parse("cat") {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("cat", items[0].job.command.as_str());
            assert_eq!(1, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_with_single_character_arguments() {
        if let Statement::Pipeline(pipeline) = parse("echo a b c") {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("echo", items[0].job.args[0].as_str());
            assert_eq!("a", items[0].job.args[1].as_str());
            assert_eq!("b", items[0].job.args[2].as_str());
            assert_eq!("c", items[0].job.args[3].as_str());
            assert_eq!(4, items[0].job.args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn job_with_args() {
        if let Statement::Pipeline(pipeline) = parse("ls -al dir") {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("ls", items[0].job.command.as_str());
            assert_eq!("-al", items[0].job.args[1].as_str());
            assert_eq!("dir", items[0].job.args[2].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn parse_empty_string() {
        if let Statement::Default = parse("") {
            ()
        } else {
            assert!(false);
        }
    }

    #[test]
    fn multiple_white_space_between_words() {
        if let Statement::Pipeline(pipeline) = parse("ls \t -al\t\tdir") {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("ls", items[0].job.command.as_str());
            assert_eq!("-al", items[0].job.args[1].as_str());
            assert_eq!("dir", items[0].job.args[2].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn trailing_whitespace() {
        if let Statement::Pipeline(pipeline) = parse("ls -al\t ") {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("ls", pipeline.items[0].job.command.as_str());
            assert_eq!("-al", pipeline.items[0].job.args[1].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn double_quoting() {
        if let Statement::Pipeline(pipeline) = parse("echo \"a > 10\" \"a < 10\"") {
            let items = pipeline.items;
            assert_eq!("\"a > 10\"", items[0].job.args[1].as_str());
            assert_eq!("\"a < 10\"", items[0].job.args[2].as_str());
            assert_eq!(3, items[0].job.args.len());
        } else {
            assert!(false)
        }
    }

    #[test]
    fn double_quoting_contains_single() {
        if let Statement::Pipeline(pipeline) = parse("echo \"Hello 'Rusty' World\"") {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\"Hello \'Rusty\' World\"", items[0].job.args[1].as_str());
        } else {
            assert!(false)
        }
    }

    #[test]
    fn multi_quotes() {
        if let Statement::Pipeline(pipeline) = parse("echo \"Hello \"Rusty\" World\"") {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\"Hello \"Rusty\" World\"", items[0].job.args[1].as_str());
        } else {
            assert!(false)
        }

        if let Statement::Pipeline(pipeline) = parse("echo \'Hello \'Rusty\' World\'") {
            let items = pipeline.items;
            assert_eq!(2, items[0].job.args.len());
            assert_eq!("\'Hello \'Rusty\' World\'", items[0].job.args[1].as_str());
        } else {
            assert!(false)
        }
    }

    #[test]
    fn all_whitespace() {
        if let Statement::Default = parse("  \t ") {
            ()
        } else {
            assert!(false);
        }
    }

    #[test]
    fn not_background_job() {
        if let Statement::Pipeline(pipeline) = parse("echo hello world") {
            let items = pipeline.items;
            assert_eq!(JobKind::Last, items[0].job.kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job() {
        if let Statement::Pipeline(pipeline) = parse("echo hello world&") {
            let items = pipeline.items;
            assert_eq!(JobKind::Background, items[0].job.kind);
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) = parse("echo hello world &") {
            let items = pipeline.items;
            assert_eq!(JobKind::Background, items[0].job.kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn disown_job() {
        if let Statement::Pipeline(pipeline) = parse("echo hello world&!") {
            let items = pipeline.items;
            assert_eq!(JobKind::Disown, items[0].job.kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn lone_comment() {
        if let Statement::Default = parse("# ; \t as!!+dfa") {
            ()
        } else {
            assert!(false);
        }
    }

    #[test]
    fn leading_whitespace() {
        if let Statement::Pipeline(pipeline) = parse("    \techo") {
            let items = pipeline.items;
            assert_eq!(1, items.len());
            assert_eq!("echo", items[0].job.command.as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_quoting() {
        if let Statement::Pipeline(pipeline) = parse("echo '#!!;\"\\'") {
            let items = pipeline.items;
            assert_eq!("'#!!;\"\\'", items[0].job.args[1].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        if let Statement::Pipeline(pipeline) =
            parse("echo 123 456 \"ABC 'DEF' GHI\" 789 one'  'two")
        {
            let items = pipeline.items;
            assert_eq!("123", items[0].job.args[1].as_str());
            assert_eq!("456", items[0].job.args[2].as_str());
            assert_eq!("\"ABC 'DEF' GHI\"", items[0].job.args[3].as_str());
            assert_eq!("789", items[0].job.args[4].as_str());
            assert_eq!("one'  'two", items[0].job.args[5].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn several_blank_lines() {
        if let Statement::Default = parse("\n\n\n") {
            ()
        } else {
            assert!(false);
        }
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection() {
        let input = "cat | echo hello | cat < stuff > other";
        if let Statement::Pipeline(pipeline) = parse(input) {
            assert_eq!(3, pipeline.items.len());
            assert_eq!("cat", pipeline.clone().items[0].job.args[0].as_str());
            assert_eq!("echo", pipeline.clone().items[1].job.args[0].as_str());
            assert_eq!("hello", pipeline.clone().items[1].job.args[1].as_str());
            assert_eq!("cat", pipeline.clone().items[2].job.args[0].as_str());
            assert_eq!(vec![Input::File("stuff".into())], pipeline.items[2].inputs);
            assert_eq!("other", pipeline.clone().items[2].outputs[0].file.as_str());
            assert!(!pipeline.clone().items[2].outputs[0].append);
            assert_eq!(input.to_owned(), pipeline.to_string());
        } else {
            assert!(false);
        }
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat < stuff >> other") {
            assert_eq!(3, pipeline.items.len());
            assert_eq!(Input::File("stuff".into()), pipeline.items[2].inputs[0]);
            assert_eq!("other", pipeline.items[2].outputs[0].file.as_str());
            assert!(pipeline.items[2].outputs[0].append);
        } else {
            assert!(false);
        }
    }

    #[test]
    // Ensures no regression for infinite loop when args() hits
    // '^' while not in the top level
    fn args_loop_terminates() {
        if let Statement::Pipeline(pipeline) = parse("$(^) '$(^)'") {
            assert_eq!("$(^)", pipeline.items[0].job.args[0].as_str());
            assert_eq!("\'$(^)\'", pipeline.items[0].job.args[1].as_str());
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
                    job:     Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  vec![
                        Input::File("file1".into()),
                        Input::HereString("\"herestring\"".into()),
                    ],
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["tr", "'x'", "'y'"], JobKind::Last),
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
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append_stderr() {
        let input = "cat | echo hello | cat < stuff ^>> other";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job:     Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["echo", "hello"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["cat"], JobKind::Last),
                    inputs:  vec![Input::File("stuff".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Stderr,
                        file:   "other".into(),
                        append: true,
                    }],
                },
            ],
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_append_both() {
        let input = "cat | echo hello | cat < stuff &>> other";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job:     Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["echo", "hello"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["cat"], JobKind::Last),
                    inputs:  vec![Input::File("stuff".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Both,
                        file:   "other".into(),
                        append: true,
                    }],
                },
            ],
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn pipeline_with_redirection_reverse_order() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat > stuff < other") {
            assert_eq!(3, pipeline.items.len());
            assert_eq!(vec![Input::File("other".into())], pipeline.items[2].inputs);
            assert_eq!("stuff", pipeline.items[2].outputs[0].file.as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn var_meets_quote() {
        if let Statement::Pipeline(pipeline) = parse("echo $x '{()}' test") {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("echo", pipeline.clone().items[0].job.args[0].as_str());
            assert_eq!("$x", pipeline.clone().items[0].job.args[1].as_str());
            assert_eq!("'{()}'", pipeline.clone().items[0].job.args[2].as_str());
            assert_eq!("test", pipeline.clone().items[0].job.args[3].as_str());
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) = parse("echo $x'{()}' test") {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("echo", pipeline.clone().items[0].job.args[0].as_str());
            assert_eq!("$x'{()}'", pipeline.clone().items[0].job.args[1].as_str());
            assert_eq!("test", pipeline.clone().items[0].job.args[2].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn herestring() {
        let input = "calc <<< $(cat math.txt)";
        let expected = Pipeline {
            items: vec![PipeItem {
                job:     Job::new(array!["calc"], JobKind::Last),
                inputs:  vec![Input::HereString("$(cat math.txt)".into())],
                outputs: vec![],
            }],
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    fn heredoc() {
        let input = "calc << EOF\n1 + 2\n3 + 4\nEOF";
        let expected = Pipeline {
            items: vec![PipeItem {
                job:     Job::new(array!["calc"], JobKind::Last),
                inputs:  vec![Input::HereString("1 + 2\n3 + 4".into())],
                outputs: vec![],
            }],
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    // FIXME: May need updating after resolution of which part of the pipe
    // the input redirection shoud be associated with.
    fn piped_herestring() {
        let input = "cat | tr 'o' 'x' <<< $VAR > out.log";
        let expected = Pipeline {
            items: vec![
                PipeItem {
                    job:     Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                    inputs:  Vec::new(),
                    outputs: Vec::new(),
                },
                PipeItem {
                    job:     Job::new(array!["tr", "'o'", "'x'"], JobKind::Last),
                    inputs:  vec![Input::HereString("$VAR".into())],
                    outputs: vec![Redirection {
                        from:   RedirectFrom::Stdout,
                        file:   "out.log".into(),
                        append: false,
                    }],
                },
            ],
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    fn awk_tests() {
        if let Statement::Pipeline(pipeline) = parse("awk -v x=$x '{ if (1) print $1 }' myfile") {
            assert_eq!(1, pipeline.items.len());
            assert_eq!("awk", pipeline.clone().items[0].job.args[0].as_str());
            assert_eq!("-v", pipeline.clone().items[0].job.args[1].as_str());
            assert_eq!("x=$x", pipeline.clone().items[0].job.args[2].as_str());
            assert_eq!(
                "'{ if (1) print $1 }'",
                pipeline.clone().items[0].job.args[3].as_str()
            );
            assert_eq!("myfile", pipeline.clone().items[0].job.args[4].as_str());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn escaped_filenames() {
        let input = "echo zardoz >> foo\\'bar";
        let expected = Pipeline {
            items: vec![PipeItem {
                job:     Job::new(array!["echo", "zardoz"], JobKind::Last),
                inputs:  Vec::new(),
                outputs: vec![Redirection {
                    from:   RedirectFrom::Stdout,
                    file:   "foo\\'bar".into(),
                    append: true,
                }],
            }],
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    fn assert_parse_error(s: &str) {
        assert!(super::Collector::new(s).parse().is_err());
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
