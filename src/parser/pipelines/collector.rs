#![allow(eq_op)] // Required as a macro sets this clippy warning off.

use std::collections::HashSet;
use std::iter::Peekable;

use super::{Input, Pipeline, RedirectFrom, Redirection};
use shell::{Job, JobKind};
use types::*;

pub struct Collector<'a> {
    data: &'a str,
}

lazy_static! {
    /// The set of bytes that will always indicate an end of an arg
    static ref FOLLOW_ARGS: HashSet<u8> = b"&|<> \t".into_iter().map(|b| *b).collect();
}

impl<'a> Collector<'a> {
    pub fn new(data: &'a str) -> Self { Collector { data } }

    pub fn run(data: &'a str) -> Result<Pipeline, &'static str> { Collector::new(data).parse() }

    fn peek(&self, index: usize) -> Option<u8> {
        if index < self.data.len() { Some(self.data.as_bytes()[index]) } else { None }
    }

    fn single_quoted<I>(&self, bytes: &mut Peekable<I>, start: usize) -> Result<&'a str, &'static str>
        where I: Iterator<Item = (usize, u8)>
    {
        while let Some(&(i, b)) = bytes.peek() {
            match b {
                // We return an inclusive range to keep the quote type intact
                b'\'' => {
                    bytes.next();
                    return Ok(&self.data[start..i + 1]);
                }
                _ => (),
            }
            bytes.next();
        }
        Err("ion: syntax error: unterminated single quote")
    }

    fn double_quoted<I>(&self, bytes: &mut Peekable<I>, start: usize) -> Result<&'a str, &'static str>
        where I: Iterator<Item = (usize, u8)>
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

    fn arg<I>(&self, bytes: &mut Peekable<I>) -> Result<Option<&'a str>, &'static str>
        where I: Iterator<Item = (usize, u8)>
    {
        // XXX: I don't think its the responsibility of the pipeline parser to do this but I'm
        // not sure of a better solution
        let mut array_level = 0;
        let mut proc_level = 0;
        let mut brace_level = 0;
        let mut start = None;
        let mut end = None;

        macro_rules! is_toplevel { () => (array_level + proc_level + brace_level == 0) }

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
                    bytes.next();
                }
                b']' => {
                    array_level -= 1;
                    bytes.next();
                }
                b'{' => {
                    brace_level += 1;
                    bytes.next();
                }
                b'}' => {
                    brace_level -= 1;
                    bytes.next();
                }
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
                        // Reaching this block means that either there is no next byte, or the next
                        // byte is none of '>' or '|', indicating that this is not the beginning of
                        // a redirection for stderr
                        bytes.next();
                    }
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
        match (start, end) {
            (Some(i), Some(j)) if i < j => Ok(Some(&self.data[i..j])),
            (Some(i), None) => Ok(Some(&self.data[i..])),
            _ => Ok(None),
        }
    }

    pub fn parse(&self) -> Result<Pipeline, &'static str> {
        let mut bytes = self.data.bytes().enumerate().peekable();
        let mut args = Array::new();
        let mut jobs: Vec<Job> = Vec::new();
        let mut input: Option<Input> = None;
        let mut outfile: Option<Redirection> = None;

        /// Attempt to create a new job given a list of collected arguments
        macro_rules! try_add_job {
            ($kind:expr) => {{
                if ! args.is_empty() {
                    jobs.push(Job::new(args.clone(), $kind));
                    args.clear();
                }
            }}
        }

        /// Attempt to create a job that redirects to some output file
        macro_rules! try_redir_out {
            ($from:expr) => {{
                try_add_job!(JobKind::Last);
                let append = if let Some(&(_, b'>')) = bytes.peek() {
                    // Consume the next byte if it is part of the redirection
                    bytes.next();
                    true
                } else {
                    false
                };
                if let Some(file) = self.arg(&mut bytes)? {
                    outfile = Some(Redirection {
                        from: $from,
                        file: file.into(),
                        append
                    });
                } else {
                    return Err("expected file argument after redirection for output");
                }
            }}
        }

        /// Add a new argument that is re
        macro_rules! push_arg {
            () => {{
                if let Some(v) = self.arg(&mut bytes)? {
                    args.push(v.into());
                }
            }}
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
                            try_add_job!(JobKind::Pipe(RedirectFrom::Both));
                        }
                        Some(&(_, b'&')) => {
                            bytes.next();
                            try_add_job!(JobKind::And);
                        }
                        Some(_) | None => {
                            try_add_job!(JobKind::Background);
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
                            try_add_job!(JobKind::Pipe(RedirectFrom::Stderr));
                        }
                        Some(_) | None => push_arg!(),
                    }
                }
                b'|' => {
                    bytes.next();
                    match bytes.peek() {
                        Some(&(_, b'|')) => {
                            bytes.next();
                            try_add_job!(JobKind::Or);
                        }
                        Some(_) | None => {
                            try_add_job!(JobKind::Pipe(RedirectFrom::Stdout));
                        }
                    }
                }
                b'>' => {
                    bytes.next();
                    try_redir_out!(RedirectFrom::Stdout);
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
                                input = Some(Input::HereString(cmd.into()));
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
                            let heredoc = heredoc.lines().collect::<Vec<&str>>();
                            // Then collect the heredoc from standard input.
                            input = Some(Input::HereString(heredoc[1..heredoc.len() - 1].join("\n")));
                        }
                    } else if let Some(file) = self.arg(&mut bytes)? {
                        // Otherwise interpret it as stdin redirection
                        input = Some(Input::File(file.into()));
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
            jobs.push(Job::new(args, JobKind::Last));
        }

        Ok(Pipeline::new(jobs, input, outfile))
    }
}

#[cfg(test)]
mod tests {
    use parser::pipelines::{Input, Pipeline, RedirectFrom, Redirection};
    use parser::statement::parse;
    use shell::{Job, JobKind};
    use shell::flow_control::Statement;
    use types::Array;

    #[test]
    fn stderr_redirection() {
        if let Statement::Pipeline(pipeline) = parse("git rev-parse --abbrev-ref HEAD ^> /dev/null") {
            assert_eq!("git", pipeline.jobs[0].args[0]);
            assert_eq!("rev-parse", pipeline.jobs[0].args[1]);
            assert_eq!("--abbrev-ref", pipeline.jobs[0].args[2]);
            assert_eq!("HEAD", pipeline.jobs[0].args[3]);

            let expected = Redirection {
                from: RedirectFrom::Stderr,
                file: "/dev/null".to_owned(),
                append: false,
            };

            assert_eq!(Some(expected), pipeline.stdout);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn braces() {
        if let Statement::Pipeline(pipeline) = parse("echo {a b} {a {b c}}") {
            let jobs = pipeline.jobs;
            assert_eq!("{a b}", jobs[0].args[1]);
            assert_eq!("{a {b c}}", jobs[0].args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn methods() {
        if let Statement::Pipeline(pipeline) = parse("echo @split(var, ', ') $join(array, ',')") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("@split(var, ', ')", jobs[0].args[1]);
            assert_eq!("$join(array, ',')", jobs[0].args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_process() {
        if let Statement::Pipeline(pipeline) = parse("echo $(echo one $(echo two) three)") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("$(echo one $(echo two) three)", jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn nested_array_process() {
        if let Statement::Pipeline(pipeline) = parse("echo @(echo one @(echo two) three)") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("@(echo one @(echo two) three)", jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn quoted_process() {
        if let Statement::Pipeline(pipeline) = parse("echo \"$(seq 1 10)\"") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("\"$(seq 1 10)\"", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn process() {
        if let Statement::Pipeline(pipeline) = parse("echo $(seq 1 10 | head -1)") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("$(seq 1 10 | head -1)", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn array_process() {
        if let Statement::Pipeline(pipeline) = parse("echo @(seq 1 10 | head -1)") {
            let jobs = pipeline.jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("@(seq 1 10 | head -1)", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_no_args() {
        if let Statement::Pipeline(pipeline) = parse("cat") {
            let jobs = pipeline.jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("cat", jobs[0].command);
            assert_eq!(1, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_with_single_character_arguments() {
        if let Statement::Pipeline(pipeline) = parse("echo a b c") {
            let jobs = pipeline.jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("a", jobs[0].args[1]);
            assert_eq!("b", jobs[0].args[2]);
            assert_eq!("c", jobs[0].args[3]);
            assert_eq!(4, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn job_with_args() {
        if let Statement::Pipeline(pipeline) = parse("ls -al dir") {
            let jobs = pipeline.jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("ls", jobs[0].command);
            assert_eq!("-al", jobs[0].args[1]);
            assert_eq!("dir", jobs[0].args[2]);
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
            let jobs = pipeline.jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("ls", jobs[0].command);
            assert_eq!("-al", jobs[0].args[1]);
            assert_eq!("dir", jobs[0].args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn trailing_whitespace() {
        if let Statement::Pipeline(pipeline) = parse("ls -al\t ") {
            assert_eq!(1, pipeline.jobs.len());
            assert_eq!("ls", pipeline.jobs[0].command);
            assert_eq!("-al", pipeline.jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn double_quoting() {
        if let Statement::Pipeline(pipeline) = parse("echo \"a > 10\" \"a < 10\"") {
            let jobs = pipeline.jobs;
            assert_eq!("\"a > 10\"", jobs[0].args[1]);
            assert_eq!("\"a < 10\"", jobs[0].args[2]);
            assert_eq!(3, jobs[0].args.len());
        } else {
            assert!(false)
        }
    }

    #[test]
    fn double_quoting_contains_single() {
        if let Statement::Pipeline(pipeline) = parse("echo \"Hello 'Rusty' World\"") {
            let jobs = pipeline.jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\"Hello \'Rusty\' World\"", jobs[0].args[1]);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn multi_quotes() {
        if let Statement::Pipeline(pipeline) = parse("echo \"Hello \"Rusty\" World\"") {
            let jobs = pipeline.jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\"Hello \"Rusty\" World\"", jobs[0].args[1]);
        } else {
            assert!(false)
        }

        if let Statement::Pipeline(pipeline) = parse("echo \'Hello \'Rusty\' World\'") {
            let jobs = pipeline.jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\'Hello \'Rusty\' World\'", jobs[0].args[1]);
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
            let jobs = pipeline.jobs;
            assert_eq!(JobKind::Last, jobs[0].kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job() {
        if let Statement::Pipeline(pipeline) = parse("echo hello world&") {
            let jobs = pipeline.jobs;
            assert_eq!(JobKind::Background, jobs[0].kind);
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) = parse("echo hello world &") {
            let jobs = pipeline.jobs;
            assert_eq!(JobKind::Background, jobs[0].kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn and_job() {
        if let Statement::Pipeline(pipeline) = parse("echo one && echo two") {
            let jobs = pipeline.jobs;
            assert_eq!(JobKind::And, jobs[0].kind);
            assert_eq!(array!["echo", "one"], jobs[0].args);
            assert_eq!(array!["echo", "two"], jobs[1].args);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn or_job() {
        if let Statement::Pipeline(pipeline) = parse("echo one || echo two") {
            let jobs = pipeline.jobs;
            assert_eq!(JobKind::Or, jobs[0].kind);
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
            let jobs = pipeline.jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("echo", jobs[0].command);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_quoting() {
        if let Statement::Pipeline(pipeline) = parse("echo '#!!;\"\\'") {
            let jobs = pipeline.jobs;
            assert_eq!("'#!!;\"\\'", jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        if let Statement::Pipeline(pipeline) = parse("echo 123 456 \"ABC 'DEF' GHI\" 789 one'  'two") {
            let jobs = pipeline.jobs;
            assert_eq!("123", jobs[0].args[1]);
            assert_eq!("456", jobs[0].args[2]);
            assert_eq!("\"ABC 'DEF' GHI\"", jobs[0].args[3]);
            assert_eq!("789", jobs[0].args[4]);
            assert_eq!("one'  'two", jobs[0].args[5]);
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
    fn pipeline_with_redirection() {
        let input = "cat | echo hello | cat < stuff > other";
        if let Statement::Pipeline(pipeline) = parse(input) {
            assert_eq!(3, pipeline.jobs.len());
            assert_eq!("cat", &pipeline.clone().jobs[0].args[0]);
            assert_eq!("echo", &pipeline.clone().jobs[1].args[0]);
            assert_eq!("hello", &pipeline.clone().jobs[1].args[1]);
            assert_eq!("cat", &pipeline.clone().jobs[2].args[0]);
            assert_eq!(Some(Input::File("stuff".into())), pipeline.stdin);
            assert_eq!("other", &pipeline.clone().stdout.unwrap().file);
            assert!(!pipeline.clone().stdout.unwrap().append);
            assert_eq!(input.to_owned(), pipeline.to_string());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipeline_with_redirection_append() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat < stuff >> other") {
            assert_eq!(3, pipeline.jobs.len());
            assert_eq!(Some(Input::File("stuff".into())), pipeline.stdin);
            assert_eq!("other", &pipeline.clone().stdout.unwrap().file);
            assert!(pipeline.clone().stdout.unwrap().append);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipeline_with_redirection_append_stderr() {
        let input = "cat | echo hello | cat < stuff ^>> other";
        let expected = Pipeline {
            jobs: vec![
                Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                Job::new(array!["echo", "hello"], JobKind::Pipe(RedirectFrom::Stdout)),
                Job::new(array!["cat"], JobKind::Last),
            ],
            stdin: Some(Input::File("stuff".into())),
            stdout: Some(Redirection {
                from: RedirectFrom::Stderr,
                file: "other".into(),
                append: true,
            }),
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    #[test]
    fn pipeline_with_redirection_append_both() {
        let input = "cat | echo hello | cat < stuff &>> other";
        let expected = Pipeline {
            jobs: vec![
                Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                Job::new(array!["echo", "hello"], JobKind::Pipe(RedirectFrom::Stdout)),
                Job::new(array!["cat"], JobKind::Last),
            ],
            stdin: Some(Input::File("stuff".into())),
            stdout: Some(Redirection {
                from: RedirectFrom::Both,
                file: "other".into(),
                append: true,
            }),
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));
    }

    #[test]
    fn pipeline_with_redirection_reverse_order() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat > stuff < other") {
            assert_eq!(3, pipeline.jobs.len());
            assert_eq!(Some(Input::File("other".into())), pipeline.stdin);
            assert_eq!("stuff", &pipeline.clone().stdout.unwrap().file);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn var_meets_quote() {
        if let Statement::Pipeline(pipeline) = parse("echo $x '{()}' test") {
            assert_eq!(1, pipeline.jobs.len());
            assert_eq!("echo", &pipeline.clone().jobs[0].args[0]);
            assert_eq!("$x", &pipeline.clone().jobs[0].args[1]);
            assert_eq!("'{()}'", &pipeline.clone().jobs[0].args[2]);
            assert_eq!("test", &pipeline.clone().jobs[0].args[3]);
        } else {
            assert!(false);
        }

        if let Statement::Pipeline(pipeline) = parse("echo $x'{()}' test") {
            assert_eq!(1, pipeline.jobs.len());
            assert_eq!("echo", &pipeline.clone().jobs[0].args[0]);
            assert_eq!("$x'{()}'", &pipeline.clone().jobs[0].args[1]);
            assert_eq!("test", &pipeline.clone().jobs[0].args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn herestring() {
        let input = "calc <<< $(cat math.txt)";
        let expected = Pipeline {
            jobs: vec![Job::new(array!["calc"], JobKind::Last)],
            stdin: Some(Input::HereString("$(cat math.txt)".into())),
            stdout: None,
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    fn heredoc() {
        let input = "calc << EOF\n1 + 2\n3 + 4\nEOF";
        let expected = Pipeline {
            jobs: vec![Job::new(array!["calc"], JobKind::Last)],
            stdin: Some(Input::HereString("1 + 2\n3 + 4".into())),
            stdout: None,
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    fn piped_herestring() {
        let input = "cat | tr 'o' 'x' <<< $VAR > out.log";
        let expected = Pipeline {
            jobs: vec![
                Job::new(array!["cat"], JobKind::Pipe(RedirectFrom::Stdout)),
                Job::new(array!["tr", "'o'", "'x'"], JobKind::Last),
            ],
            stdin: Some(Input::HereString("$VAR".into())),
            stdout: Some(Redirection {
                from: RedirectFrom::Stdout,
                file: "out.log".into(),
                append: false,
            }),
        };
        assert_eq!(Statement::Pipeline(expected), parse(input));
    }

    #[test]
    fn awk_tests() {
        if let Statement::Pipeline(pipeline) = parse("awk -v x=$x '{ if (1) print $1 }' myfile") {
            assert_eq!(1, pipeline.jobs.len());
            assert_eq!("awk", &pipeline.clone().jobs[0].args[0]);
            assert_eq!("-v", &pipeline.clone().jobs[0].args[1]);
            assert_eq!("x=$x", &pipeline.clone().jobs[0].args[2]);
            assert_eq!("'{ if (1) print $1 }'", &pipeline.clone().jobs[0].args[3]);
            assert_eq!("myfile", &pipeline.clone().jobs[0].args[4]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn escaped_filenames() {
        let input = "echo zardoz >> foo\\'bar";
        let expected = Pipeline {
            jobs: vec![Job::new(array!["echo", "zardoz"], JobKind::Last)],
            stdin: None,
            stdout: Some(Redirection {
                from: RedirectFrom::Stdout,
                file: "foo\\'bar".into(),
                append: true,
            }),
        };
        assert_eq!(parse(input), Statement::Pipeline(expected));

    }

}
