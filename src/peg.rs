use super::flow_control::Statement;

use std::process::Command;

use self::grammar::parse_;

use glob::glob;


#[derive(Debug, PartialEq, Clone)]
pub struct Redirection {
    pub file: String,
    pub append: bool
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pipeline {
    pub jobs: Vec<Job>,
    pub stdout: Option<Redirection>,
    pub stdin: Option<Redirection>,
}

impl Pipeline {

    pub fn new(jobs: Vec<Job>, stdin: Option<Redirection>, stdout: Option<Redirection>) -> Self {
        Pipeline {
            jobs: jobs,
            stdin: stdin,
            stdout: stdout,
        }
    }

    pub fn expand_globs(&mut self) {
        let jobs = self.jobs.drain(..).map(|mut job| {
            job.expand_globs();
            job
        }).collect();
        self.jobs = jobs;
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
    pub background: bool,
}

impl Job {

    pub fn new(args: Vec<String>, background: bool) -> Self {
        let command = args[0].clone();
        Job {
            command: command,
            args: args,
            background: background,
        }
    }

    pub fn expand_globs(&mut self) {
        let mut new_args: Vec<String> = vec![];
        for arg in self.args.drain(..) {
            let mut pushed_glob = false;
            if arg.contains(|chr| chr == '?' || chr == '*' || chr == '[') {
                if let Ok(expanded) = glob(&arg) {
                    for path in expanded.filter_map(Result::ok) {
                        pushed_glob = true;
                        new_args.push(path.to_string_lossy().into_owned());
                    }
                }
            }
            if !pushed_glob {
                new_args.push(arg);
            }
        }
        self.args = new_args;
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.command);
        for i in 1..self.args.len() {
            if let Some(arg) = self.args.get(i) {
                command.arg(arg);
            }
        }
        command
    }
}

pub fn parse(code: &str) -> Statement {
    match parse_(code) {
		Ok(code_ok) => code_ok,
		Err(err) => {
			println!("ion: Syntax {}",err);
			Statement::Pipelines(vec![])
		}
	}
}

peg_file! grammar("grammar.rustpeg");

const BACKSLASH:    u8 = 1;
const SINGLE_QUOTE: u8 = 2;
const DOUBLE_QUOTE: u8 = 4;
const WHITESPACE:   u8 = 8;
const COMMENT:      u8 = 16;
const PROCESS_ONE:  u8 = 64;
const PROCESS_TWO:  u8 = 128;

// Only valid if `SINGLE_QUOTE` and `DOUBLE_QUOTE` are not enabled
const PROCESS_VAL:  u8 = 255 ^ (BACKSLASH + WHITESPACE + COMMENT + 32);

// Determines if the character is not quoted and isn't process matched. `flags & IS_VALID` returns 0 if true
const IS_VALID: u8 = 255 ^ (BACKSLASH + WHITESPACE + COMMENT);

/// An iterator that splits a given command into pipelines -- individual command statements delimited by ';'.
struct PipelineIterator<'a> {
    match_str:       &'a str,
    flags:           u8,
    index_start:     usize,
    index_end:       usize,
    white_pos:       usize,
}

impl<'a> PipelineIterator<'a> {
    fn new(match_str: &'a str) -> PipelineIterator<'a> {
        PipelineIterator {
            match_str:       match_str,
            flags:           if match_str.chars().next().unwrap() == '#' { COMMENT } else { 0 },
            index_start:     0,
            index_end:       0,
            white_pos:       0,
        }
    }
}

impl<'a> Iterator for PipelineIterator<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        for character in self.match_str.chars().skip(self.index_end) {
            if self.flags & COMMENT != 0 {
                self.index_end += 1;
                if character == '\n' {
                    self.flags &= 255 ^ COMMENT;
                    self.index_start = self.index_end;
                }
            } else {
                match character {
                    _ if self.flags & BACKSLASH != 0                        => self.flags ^= BACKSLASH,
                    '\\'                                                    => self.flags |= BACKSLASH,
                    '\'' if self.flags & (PROCESS_TWO + DOUBLE_QUOTE) == 0  => self.flags ^= SINGLE_QUOTE,
                    '"'  if self.flags & (PROCESS_TWO + SINGLE_QUOTE) == 0  => self.flags ^= DOUBLE_QUOTE,
                    '$'  if self.flags & PROCESS_VAL == 0                   => self.flags |= PROCESS_ONE,
                    '('  if self.flags & PROCESS_VAL == PROCESS_ONE         => self.flags ^= PROCESS_ONE + PROCESS_TWO,
                    ')'  if self.flags & PROCESS_VAL == PROCESS_TWO         => self.flags &= 255 ^ PROCESS_TWO,
                    '#'  if self.flags == WHITESPACE => {
                        if self.index_start < self.white_pos {
                            let command = &self.match_str[self.index_start..self.white_pos];
                            self.index_start = self.index_end + 1;
                            self.flags |= COMMENT;
                            self.index_end += 1;
                            return Some(command)
                        } else {
                            self.index_start = self.index_end + 1;
                            self.flags |= COMMENT;
                        }
                    },
                    ' ' | '\t' if self.flags & IS_VALID == 0 => {
                        if self.index_start == self.index_end { self.index_start += 1; }
                        self.flags |= WHITESPACE;
                        if self.white_pos == 0 { self.white_pos = self.index_end; }
                        self.index_end += 1;
                        continue
                    },
                    ';' | '\n' | '\r' if self.flags & IS_VALID == 0 => {
                        if self.index_start == self.index_end {
                            self.index_start += 1;
                            self.flags |= WHITESPACE;
                            if self.white_pos == 0 { self.white_pos = self.index_end; }
                        } else {
                            let command = &self.match_str[self.index_start..self.index_end];
                            self.index_start = self.index_end + 1;
                            self.flags |= WHITESPACE;
                            if self.white_pos == 0 { self.white_pos = self.index_end; }
                            if command.chars().any(|x| x != ' ' && x != '\n' && x != '\r' && x != '\t') {
                                self.index_end += 1;
                                return Some(command);
                            }
                        }
                        self.index_end += 1;
                        continue
                    },
                    _ if (self.flags >> 6 != 2) => self.flags &= 255 ^ (PROCESS_ONE + PROCESS_TWO),
                    _ => (),
                }
                self.flags &= 255 ^ WHITESPACE;
                self.white_pos = 0;
                self.index_end += 1;
            }
        }

        if self.flags & COMMENT == 0 && self.match_str.len() > self.index_start {
            let command = &self.match_str[self.index_start..];
            self.index_start = self.match_str.len() + 1;
            if command.chars().any(|x| x != ' ' && x != '\n' && x != '\r' && x != '\t') {
                Some(command)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(PartialEq)]
enum RedirMode { False, Stdin, Stdout, StdoutAppend }

/// Parses each individual pipeline, separating arguments, pipes, background tasks, and redirections.
pub fn collect_pipelines(pipelines: &mut Vec<Pipeline>, possible_error: &mut Option<&str>, command: &str) {
    for args in PipelineIterator::new(command) {
        let mut jobs: Vec<Job> = Vec::new();
        let mut args_iter = args.chars();
        let (mut index, mut arg_start) = (0, 0);
        let mut flags = 0u8; // (backslash, single_quote, double_quote, x, x, x, process_one, process_two)

        let mut arguments: Vec<String> = Vec::new();

        let (mut in_file, mut out_file) = (None, None);
        let mut mode = RedirMode::False;

        macro_rules! redir_check {
            ($file:ident, $name:ident, $is_append:expr) => {{
                if $file.is_none() {
                    if $name.is_empty() {
                        *possible_error = Some("missing standard output file argument after '>'");
                    } else {
                        $file = Some(Redirection { file: $name, append: $is_append });
                    }
                }
            }}
        }

        'outer: loop {

            macro_rules! redir_found {
                ($kind:expr) => {{ mode = $kind; index += 1; arg_start = index; continue 'outer }}
            }

            macro_rules! job_found {
                ($is_background:expr) => {{
                    if arguments.is_empty() {
                        jobs.push(Job::new(vec![args[arg_start..index].to_owned()], $is_background));
                    } else {
                        jobs.push(Job::new(arguments.clone(), $is_background));
                        arguments.clear();
                    }
                    arg_start = index + 1;
                }}
            }

            match mode {
                RedirMode::False => {
                    while let Some(character) = args_iter.next() {
                        match character {
                            _ if flags & BACKSLASH != 0                => flags ^= BACKSLASH,
                            '\\'                                       => flags ^= BACKSLASH,
                            '$'  if flags & PROCESS_VAL == 0           => flags |= PROCESS_ONE,
                            '('  if flags & PROCESS_VAL == PROCESS_ONE => flags ^= PROCESS_ONE + PROCESS_TWO,
                            ')'  if flags & PROCESS_VAL == PROCESS_TWO => flags &= 255 ^ PROCESS_TWO,
                            '\''                                       => flags ^= SINGLE_QUOTE,
                            '"'                                        => flags ^= DOUBLE_QUOTE,
                            ' ' | '\t' if (flags & IS_VALID == 0) => {
                                if arg_start != index {
                                    arguments.push(args[arg_start..index].to_owned());
                                    arg_start = index + 1;
                                } else {
                                    arg_start += 1;
                                }
                            },
                            '|' if (flags & IS_VALID == 0) => job_found!(false),
                            '&' if (flags & IS_VALID == 0) => job_found!(true),
                            '>' if (flags & IS_VALID == 0) => redir_found!(RedirMode::Stdout),
                            '<' if (flags & IS_VALID == 0) => redir_found!(RedirMode::Stdin),
                            _   if (flags >> 6 != 2)       => flags &= 255 ^ (PROCESS_ONE + PROCESS_TWO),
                            _ => (),
                        }
                        index += 1;
                    }
                    break 'outer
                },
                RedirMode::Stdout | RedirMode::StdoutAppend => {
                    match args_iter.next() {
                        Some(character) => if character == '>' { mode = RedirMode::StdoutAppend; },
                        None => {
                            *possible_error = Some("missing standard output file argument after '>'");
                            break 'outer
                        }
                    }

                    let mut stdout_file = String::new();
                    let mut found_file = false;
                    while let Some(character) = args_iter.next() {
                        if found_file {
                            if character == '<' {
                                if in_file.is_some() {
                                    break 'outer
                                } else {
                                    mode = RedirMode::Stdin;
                                    continue 'outer
                                }
                            }
                        } else {
                            match character {
                                _ if flags & BACKSLASH != 0 => {
                                    stdout_file.push(character);
                                    flags ^= BACKSLASH;
                                }
                                '\\' => flags ^= BACKSLASH,
                                ' ' | '\t' | '|' if stdout_file.is_empty() => (),
                                ' ' | '\t' | '|' => {
                                    found_file = true;
                                    out_file = Some(Redirection {
                                        file: stdout_file.clone(),
                                        append: mode == RedirMode::StdoutAppend
                                    });
                                },
                                '<' if stdout_file.is_empty() => {
                                    *possible_error = Some("missing standard output file argument after '>'");
                                    break 'outer
                                }
                                '<' => {
                                    out_file = Some(Redirection {
                                        file: stdout_file.clone(),
                                        append: mode == RedirMode::StdoutAppend
                                    });

                                    if in_file.is_some() {
                                        break 'outer
                                    } else {
                                        mode = RedirMode::Stdin;
                                        continue 'outer
                                    }
                                },
                                _ => stdout_file.push(character),
                            }
                        }
                    }

                    redir_check!(out_file, stdout_file, mode == RedirMode::StdoutAppend);

                    break 'outer
                },
                RedirMode::Stdin => {
                    let mut stdin_file = String::new();
                    let mut found_file = false;

                    while let Some(character) = args_iter.next() {
                        if found_file {
                            if character == '>' {
                                if out_file.is_some() {
                                    break 'outer
                                } else {
                                    mode = RedirMode::Stdout;
                                    continue 'outer
                                }
                            }
                        } else {
                            match character {
                                _ if flags & BACKSLASH != 0 => {
                                    stdin_file.push(character);
                                    flags ^= BACKSLASH;
                                }
                                '\\' => flags ^= BACKSLASH,
                                ' ' | '\t' | '|' if stdin_file.is_empty() => (),
                                ' ' | '\t' | '|' => {
                                    found_file = true;
                                    in_file = Some(Redirection { file: stdin_file.clone(), append: false });
                                },
                                '>' if stdin_file.is_empty() => {
                                    *possible_error = Some("missing standard input file argument after '<'");
                                    break 'outer
                                }
                                '>' => {
                                    in_file = Some(Redirection { file: stdin_file.clone(), append: false });

                                    if out_file.is_some() {
                                        break 'outer
                                    } else {
                                        mode = RedirMode::Stdin;
                                        continue 'outer
                                    }
                                },
                                _ => stdin_file.push(character),
                            }
                        }
                    }

                    redir_check!(in_file, stdin_file, false);

                    break 'outer
                }
            }
        }

        if arg_start != index {
            arguments.push(args[arg_start..index].to_owned());
        }

        if !arguments.is_empty() {
            jobs.push(Job::new(arguments, false));
        }

        pipelines.push(Pipeline::new(jobs, in_file, out_file));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::grammar::*;
    use flow_control::{Statement, Comparitor};

    #[test]
    fn quoted_process_with_extra_commands() {
        if let Statement::Pipelines(mut pipelines) = parse("let A = \"$(seq 1 10)\"; echo $A; echo \"$A\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("let", jobs[0].args[0]);
            assert_eq!("A", jobs[0].args[1]);
            assert_eq!("=", jobs[0].args[2]);
            assert_eq!("\"$(seq 1 10)\"", jobs[0].args[3]);
            assert_eq!(4, jobs[0].args.len());
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("$A", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("\"$A\"", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn process_with_extra_commands() {
        if let Statement::Pipelines(mut pipelines) = parse("let A = $(seq 1 10); echo $A; echo \"$A\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("let", jobs[0].args[0]);
            assert_eq!("A", jobs[0].args[1]);
            assert_eq!("=", jobs[0].args[2]);
            assert_eq!("$(seq 1 10)", jobs[0].args[3]);
            assert_eq!(4, jobs[0].args.len());
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("$A", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("\"$A\"", jobs[0].args[1]);
            assert_eq!(2, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipes_without_spaces() {
        if let Statement::Pipelines(mut pipelines) = parse("ls|cat; ls | cat|cat") {
            let jobs = pipelines.remove(0).jobs;
            println!("{:?}", jobs);
            assert_eq!("ls", jobs[0].args[0]);
            assert_eq!(1, jobs[0].args.len());
            assert_eq!("cat", jobs[1].args[0]);
            assert_eq!(1, jobs[1].args.len());
            let jobs = pipelines.remove(0).jobs;
            println!("{:?}", jobs);
            assert_eq!("ls", jobs[0].args[0]);
            assert_eq!(1, jobs[0].args.len());
            assert_eq!("cat", jobs[1].args[0]);
            assert_eq!(1, jobs[1].args.len());
            assert_eq!("cat", jobs[2].args[0]);
            assert_eq!(1, jobs[2].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_no_args() {
        if let Statement::Pipelines(mut pipelines) = parse("cat") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("cat", jobs[0].command);
            assert_eq!(1, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_job_with_single_character_arguments() {
        if let Statement::Pipelines(mut pipelines) = parse("echo a b c") {
            let jobs = pipelines.remove(0).jobs;
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
    fn single_job_with_args() {
        if let Statement::Pipelines(mut pipelines) = parse("ls -al dir") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("ls", jobs[0].command);
            assert_eq!("-al", jobs[0].args[1]);
            assert_eq!("dir", jobs[0].args[2]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn multiple_jobs_with_args() {
        if let Statement::Pipelines(pipelines) = parse("ls -al;cat tmp.txt") {
            assert_eq!(2, pipelines.len());
            assert_eq!("ls", pipelines[0].jobs[0].command);
            assert_eq!("-al", pipelines[0].jobs[0].args[1]);
            assert_eq!("cat", pipelines[1].jobs[0].command);
            assert_eq!("tmp.txt", pipelines[1].jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn parse_empty_string() {
        if let Statement::Pipelines(pipelines) = parse("") {
            assert_eq!(0, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn multiple_white_space_between_words() {
        if let Statement::Pipelines(mut pipelines) = parse("ls \t -al\t\tdir") {
            let jobs = pipelines.remove(0).jobs;
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
        if let Statement::Pipelines(pipelines) = parse("ls -al\t ") {
            assert_eq!(1, pipelines.len());
            assert_eq!("ls", pipelines[0].jobs[0].command);
            assert_eq!("-al", pipelines[0].jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn double_quoting() {
        if let Statement::Pipelines(mut pipelines) = parse("echo \"Hello World\" \"From Rust\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(3, jobs[0].args.len());
            assert_eq!("\"Hello World\"", jobs[0].args[1]);
            assert_eq!("\"From Rust\"", jobs[0].args[2]);
        } else {
            assert!(false)
        }
    }



    #[test]
    fn double_quoting_contains_single() {
        if let Statement::Pipelines(mut pipelines) = parse("echo \"Hello 'Rusty' World\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\"Hello \'Rusty\' World\"", jobs[0].args[1]);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn multi_quotes() {
        if let Statement::Pipelines(mut pipelines) = parse("echo \"Hello \"Rusty\" World\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\"Hello \"Rusty\" World\"", jobs[0].args[1]);
        } else {
            assert!(false)
        }

        if let Statement::Pipelines(mut pipelines) = parse("echo \'Hello \'Rusty\' World\'") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(2, jobs[0].args.len());
            assert_eq!("\'Hello \'Rusty\' World\'", jobs[0].args[1]);
        } else {
            assert!(false)
        }
    }

    #[test]
    fn all_whitespace() {
        if let Statement::Pipelines(pipelines) = parse("  \t ") {
            assert_eq!(0, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn not_background_job() {
        if let Statement::Pipelines(mut pipelines) = parse("echo hello world") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(false, jobs[0].background);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job() {
        if let Statement::Pipelines(mut pipelines) = parse("echo hello world&") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(true, jobs[0].background);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job_with_space() {
        if let Statement::Pipelines(mut pipelines) = parse("echo hello world &") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(true, jobs[0].background);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn lone_comment() {
        if let Statement::Pipelines(pipelines) = parse("# ; \t as!!+dfa") {
            assert_eq!(0, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn command_followed_by_comment() {
        if let Statement::Pipelines(pipelines) = parse("cat # ; \t as!!+dfa") {
            assert_eq!(1, pipelines.len());
            assert_eq!(1, pipelines[0].jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn comments_in_multiline_script() {
        if let Statement::Pipelines(pipelines) = parse("echo\n# a comment;\necho#asfasdf") {
            assert_eq!(2, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn multiple_newlines() {
        if let Statement::Pipelines(pipelines) = parse("echo\n\ncat") {
            assert_eq!(2, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn leading_whitespace() {
        if let Statement::Pipelines(mut pipelines) = parse("    \techo") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(1, jobs.len());
            assert_eq!("echo", jobs[0].command);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn indentation_on_multiple_lines() {
        if let Statement::Pipelines(pipelines) = parse("echo\n  cat") {
            assert_eq!(2, pipelines.len());
            assert_eq!("echo", pipelines[0].jobs[0].command);
            assert_eq!("cat", pipelines[1].jobs[0].command);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn single_quoting() {
        if let Statement::Pipelines(mut pipelines) = parse("echo '#!!;\"\\'") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("'#!!;\"\\'", jobs[0].args[1]);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        if let Statement::Pipelines(mut pipelines) = parse("echo 123 456 \"ABC 'DEF' GHI\" 789 one'  'two") {
            let jobs = pipelines.remove(0).jobs;
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
        if let Statement::Pipelines(pipelines) = parse("\n\n\n") {
            assert_eq!(0, pipelines.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipelines_with_redirection() {
        if let Statement::Pipelines(pipelines) = parse("cat | echo hello | cat < stuff > other") {
            assert_eq!(3, pipelines[0].jobs.len());
            assert_eq!("cat", &pipelines[0].clone().jobs[0].args[0]);
            assert_eq!("echo", &pipelines[0].clone().jobs[1].args[0]);
            assert_eq!("hello", &pipelines[0].clone().jobs[1].args[1]);
            assert_eq!("cat", &pipelines[0].clone().jobs[2].args[0]);
            assert_eq!("stuff", &pipelines[0].clone().stdin.unwrap().file);
            assert_eq!("other", &pipelines[0].clone().stdout.unwrap().file);
            assert!(!pipelines[0].clone().stdout.unwrap().append);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipeline_with_redirection_append() {
        if let Statement::Pipelines(pipelines) = parse("cat | echo hello | cat < stuff >> other") {
        assert_eq!(3, pipelines[0].jobs.len());
        assert_eq!("stuff", &pipelines[0].clone().stdin.unwrap().file);
        assert_eq!("other", &pipelines[0].clone().stdout.unwrap().file);
        assert!(pipelines[0].clone().stdout.unwrap().append);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipelines_with_redirection_reverse_order() {
        if let Statement::Pipelines(pipelines) = parse("cat | echo hello | cat > stuff < other") {
            assert_eq!(3, pipelines[0].jobs.len());
            assert_eq!("other", &pipelines[0].clone().stdin.unwrap().file);
            assert_eq!("stuff", &pipelines[0].clone().stdout.unwrap().file);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn full_script() {
        pipelines(r#"if a == a
  echo true a == a

  if b != b
    echo true b != b
  else
    echo false b != b

    if 3 > 2
      echo true 3 > 2
    else
      echo false 3 > 2
    fi
  fi
else
  echo false a == a
fi
"#)
            .unwrap();  // Make sure it parses
    }

    #[test]
    fn leading_and_trailing_junk() {
        pipelines(r#"

# comment
   # comment


    if a == a
  echo true a == a  # Line ending commment

  if b != b
    echo true b != b
  else
    echo false b != b

    if 3 > 2
      echo true 3 > 2
    else
      echo false 3 > 2
    fi
  fi
else
  echo false a == a
      fi

# comment

"#).unwrap();  // Make sure it parses
    }
    #[test]
    fn parsing_ifs() {
        // Default case where spaced normally
        let parsed_if = if_("if 1 == 2").unwrap();
        let correct_parse = Statement::If{left: "1".to_string(),
                                        comparitor: Comparitor::Equal,
                                        right: "2".to_string()};
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = if_("if 1 == 2         ").unwrap();
        let correct_parse = Statement::If{left: "1".to_string(),
                                        comparitor: Comparitor::Equal,
                                        right: "2".to_string()};
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = if_("if 1 <= 2").unwrap();
        let correct_parse = Statement::If{left: "1".to_string(),
                                        comparitor: Comparitor::LessThanOrEqual,
                                        right: "2".to_string()};
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let parsed_if = else_("else").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = else_("else         ").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = else_("         else").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = end_("end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = end_("end         ").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = end_("         end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = fn_("fn bob").unwrap();
        let correct_parse = Statement::Function{name: "bob".to_string(), args: vec!()};
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = fn_("fn bob        ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = fn_("         fn bob").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = fn_("fn bob a b").unwrap();
        let correct_parse = Statement::Function{name: "bob".to_string(), args: vec!("a".to_string(), "b".to_string())};
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = fn_("fn bob a b       ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = fn_("         fn bob a b").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_fors() {
        // Default case where spaced normally
        let parsed_if = for_("for i in a b").unwrap();
        let correct_parse = Statement::For{variable: "i".to_string(), values: vec!("a".to_string(), "b".to_string())};
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = for_("for i in a b        ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = for_("         for i in a b").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }
}
