#![allow(eq_op)] // Required as a macro sets this clippy warning off.

// TODO:
// - Rewrite this module like the shell_expand::words module
// - Implement Herestrings
// - Implement Heredocs
// - Fix the cyclomatic complexity issue

use parser::peg::{Pipeline, Redirection, RedirectFrom};
use shell::{Job, JobKind};
use types::*;

bitflags! {
    pub struct NormalFlags : u8 {
        const BACKSLASH = 1;
        const SINGLE_QUOTE = 2;
        const DOUBLE_QUOTE = 4;
        const ARRAY_PROCESS = 8;
        const METHOD = 16;
        const PROCESS_TWO = 32;
        const IS_VALID = SINGLE_QUOTE.bits
                       | METHOD.bits
                       | PROCESS_TWO.bits
                       | ARRAY_PROCESS.bits;
    }
}

bitflags! {
    pub struct VariableFlags : u8 {
        const ARRAY = 1;
        const VARIABLE = 2;
        const ARRAY_CHAR_FOUND = 4;
        const VAR_CHAR_FOUND = 8;
    }
}

#[derive(PartialEq)]
enum RedirMode { False, Stdin, Stdout(RedirectFrom), StdoutAppend(RedirectFrom) }

/// Determine which type of job will be selected based on the next character.
///
/// - If the `|` char was found and the next character is `|`, it's `Or`
/// - If the `&` char was found and the next character is `&`, it's `And`
fn get_job_kind(args: &str, index: usize, pipe_char_was_found: bool) -> (JobKind, bool) {
    if pipe_char_was_found {
        if args.bytes().nth(index) == Some(b'|') {
            (JobKind::Or, true)
        } else {
            (JobKind::Pipe(RedirectFrom::Stdout), false)
        }
    } else if args.bytes().nth(index) == Some(b'&') {
        (JobKind::And, true)
    } else {
        (JobKind::Background, false)
    }
}

#[allow(cyclomatic_complexity)]
/// Parses each individual pipeline, separating arguments, pipes, background tasks, and redirections.
pub fn collect(possible_error: &mut Option<&str>, args: &str) -> Pipeline {
    let mut jobs: Vec<Job> = Vec::new();
    let mut args_iter = args.bytes().peekable();
    let (mut index, mut arg_start) = (0, 0);
    let mut flags = NormalFlags::empty(); // (backslash, single_quote, double_quote, x, x, x, process_one, process_two)
    let mut flags_ext = VariableFlags::empty();

    let mut arguments = Array::new();

    let (mut in_file, mut out_file) = (None, None);
    let mut mode = RedirMode::False;
    let (mut levels, mut array_levels, mut array_process_levels) = (0, 0, 0);

    macro_rules! redir_check {
        ($from:expr, $file:ident, $name:ident, $is_append:expr) => {{
            if $file.is_none() {
                if $name.is_empty() {
                    *possible_error = Some("missing standard output file argument after '>'");
                } else {
                    $file = Some(Redirection {
                        from:   $from,
                        file:   unsafe { String::from_utf8_unchecked($name) },
                        append: $is_append
                    });
                }
            }
        }}
    }

    'outer: loop {

        macro_rules! redir_found {
            ($kind:expr) => {{ mode = $kind; index += 1; arg_start = index; continue 'outer }}
        }

        macro_rules! job_found {
            ($from:expr, $pipe_char_was_found:expr) => {{

                let (kind, advance) = match $from {
                    RedirectFrom::Stdout => get_job_kind(args, index+1, $pipe_char_was_found),
                    _ => {
                        arg_start += 1;
                        index += 1;
                        (JobKind::Pipe($from), true)
                    }
                };

                // If either `And` or `Or` was found, advance the iterator once.
                if advance { let _ = args_iter.next(); }

                if arguments.is_empty() {
                    jobs.push(
                        Job::new(
                            Some(args[arg_start..index].into()).into_iter().collect(),
                            kind
                        )
                    );
                } else {
                    let byte_index = if $from == RedirectFrom::Stdout { index-1 } else { index-2 };
                    if args.as_bytes()[byte_index] != b' ' {
                        arguments.push(args[arg_start..index].into());
                    }
                    jobs.push(Job::new(arguments.clone(), kind));
                    arguments.clear();
                }
                if advance { index += 1; }
                arg_start = index + 1;
            }}
        }

        match mode {
            RedirMode::False => {
                while let Some(character) = args_iter.next() {
                    match character {
                        _ if flags.contains(BACKSLASH) => flags ^= BACKSLASH,
                        b'\\'                          => flags ^= BACKSLASH,
                        b'@'                           => {
                            flags_ext |= ARRAY | ARRAY_CHAR_FOUND;
                            index += 1;
                            continue
                        },
                        b'$' if !flags.contains(IS_VALID) => {
                            flags_ext |= VARIABLE | VAR_CHAR_FOUND;
                            index += 1;
                            continue
                        },
                        b'['                      => array_levels += 1,
                        b']' if array_levels != 0 => array_levels -= 1,
                        b'(' if flags_ext.contains(VAR_CHAR_FOUND) => {
                            flags |= PROCESS_TWO;
                            levels += 1;
                        },
                        b'(' if flags_ext.contains(ARRAY_CHAR_FOUND) => {
                            flags |= ARRAY_PROCESS;
                            array_process_levels += 1;
                        },
                        b'(' if flags_ext.intersects(VARIABLE | ARRAY) => {
                            flags |= METHOD;
                            flags_ext -= VARIABLE | ARRAY;
                        },
                        b')' if levels == 0 && flags.contains(METHOD) && !flags.contains(SINGLE_QUOTE) => {
                            flags -= METHOD;
                        }
                        b')' if flags.contains(ARRAY_PROCESS) => array_process_levels -= 1,
                        b')' if flags.contains(PROCESS_TWO) => {
                            levels -= 0;
                            if levels == 0 { flags -= PROCESS_TWO; }
                        },
                        b'\'' => { flags ^= SINGLE_QUOTE; flags_ext -= VARIABLE | ARRAY; },
                        b'"'  => { flags ^= DOUBLE_QUOTE; flags_ext -= VARIABLE | ARRAY; },
                        b' ' | b'\t' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 =>
                        {
                            if arg_start != index {
                                arguments.push(
                                    args[arg_start..index].into()
                                );
                                arg_start = index + 1;
                            } else {
                                arg_start += 1;
                            }
                        },
                        b'|' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 => job_found!(RedirectFrom::Stdout, true),
                        b'&' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 => {
                            match args_iter.peek() {
                                Some(&b'>') => {
                                    let _ = args_iter.next();
                                    redir_found!(RedirMode::Stdout(RedirectFrom::Both));
                                },
                                _ => job_found!(RedirectFrom::Stdout, false)
                            }
                        },
                        b'^' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 => {
                            match args_iter.peek() {
                                Some(&b'>') => {
                                    let _ = args_iter.next();
                                    redir_found!(RedirMode::Stdout(RedirectFrom::Stderr));
                                },
                                Some(&b'|') => {
                                    let _ = args_iter.next();
                                    job_found!(RedirectFrom::Stderr, true);
                                }
                                _ => ()
                            }
                        },
                        b'>' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 => redir_found!(RedirMode::Stdout(RedirectFrom::Stdout)),
                        b'<' if !flags.intersects(DOUBLE_QUOTE | IS_VALID) && array_levels == 0
                            && array_process_levels == 0 => redir_found!(RedirMode::Stdin),
                        0...47 | 58...64 | 91...94 | 96 | 123...127 => flags_ext -= VARIABLE | ARRAY,
                        _ => (),
                    }
                    flags_ext -= VAR_CHAR_FOUND | ARRAY_CHAR_FOUND;
                    index += 1;
                }
                break 'outer
            },
            RedirMode::Stdout(from) | RedirMode::StdoutAppend(from) => {
                match args_iter.next() {
                    Some(character) => if character == b'>' { mode = RedirMode::StdoutAppend(from); },
                    None => {
                        *possible_error = Some("missing standard output file argument after '>'");
                        break 'outer
                    }
                }

                let mut stdout_file = Vec::new();
                let mut found_file = false;
                while let Some(character) = args_iter.next() {
                    if found_file {
                        if character == b'<' {
                            if in_file.is_some() {
                                break 'outer
                            } else {
                                mode = RedirMode::Stdin;
                                continue 'outer
                            }
                        }
                    } else {
                        match character {
                            _ if flags.contains(BACKSLASH) => {
                                stdout_file.push(character);
                                flags ^= BACKSLASH;
                            }
                            b'\\' => flags ^= BACKSLASH,
                            b' ' | b'\t' | b'|' if stdout_file.is_empty() => (),
                            b' ' | b'\t' | b'|' => {
                                found_file = true;
                                out_file = Some(Redirection {
                                    from: RedirectFrom::Stdout,
                                    file: unsafe { String::from_utf8_unchecked(stdout_file.clone()) },
                                    append: if let RedirMode::StdoutAppend(_) = mode { true } else { false }
                                });
                            },
                            b'<' if stdout_file.is_empty() => {
                                *possible_error = Some("missing standard output file argument after '>'");
                                break 'outer
                            }
                            b'<' => {
                                out_file = Some(Redirection {
                                    from: RedirectFrom::Stdout,
                                    file: unsafe { String::from_utf8_unchecked(stdout_file.clone()) },
                                    append: if let RedirMode::StdoutAppend(_) = mode { true } else { false }
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

                redir_check!(
                    from,
                    out_file,
                    stdout_file,
                    if let RedirMode::StdoutAppend(_) = mode { true } else { false }
                );

                break 'outer
            },
            RedirMode::Stdin => {
                let mut stdin_file = Vec::new();
                let mut found_file = false;

                while let Some(character) = args_iter.next() {
                    if found_file {
                        if character == b'>' {
                            if out_file.is_some() {
                                break 'outer
                            } else {
                                mode = RedirMode::Stdout(RedirectFrom::Stdout);
                                continue 'outer
                            }
                        }
                    } else {
                        match character {
                            _ if flags.intersects(BACKSLASH) => {
                                stdin_file.push(character);
                                flags ^= BACKSLASH;
                            }
                            b'\\' => flags ^= BACKSLASH,
                            b' ' | b'\t' | b'|' if stdin_file.is_empty() => (),
                            b' ' | b'\t' | b'|' => {
                                found_file = true;
                                in_file = Some(Redirection {
                                    from: RedirectFrom::Stdout,
                                    file: unsafe { String::from_utf8_unchecked(stdin_file.clone()) },
                                    append: false
                                });
                            },
                            b'>' if stdin_file.is_empty() => {
                                *possible_error = Some("missing standard input file argument after '<'");
                                break 'outer
                            }
                            b'>' => {
                                in_file = Some(Redirection {
                                    from: RedirectFrom::Stdout,
                                    file: unsafe { String::from_utf8_unchecked(stdin_file.clone()) },
                                    append: false
                                });

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

                let dummy_val = RedirectFrom::Stdout;
                redir_check!(dummy_val, in_file, stdin_file, false);

                break 'outer
            }
        }
    }

    if arg_start != index {
        arguments.push(args[arg_start..].into());
    }

    if !arguments.is_empty() {
        jobs.push(Job::new(arguments, JobKind::Last));
    }

    Pipeline::new(jobs, in_file, out_file)
}

#[cfg(test)]
mod tests {
    use shell::flow_control::Statement;
    use parser::peg::{parse, RedirectFrom, Redirection};
    use shell::JobKind;
    use types::*;

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
                append: false
            };

            assert_eq!(Some(expected), pipeline.stdout);
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
            assert_eq!(Array::from_vec(vec![String::from("echo"), String::from("one")]), jobs[0].args);
            assert_eq!(Array::from_vec(vec![String::from("echo"), String::from("two")]), jobs[1].args);
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
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat < stuff > other") {
            assert_eq!(3, pipeline.jobs.len());
            assert_eq!("cat", &pipeline.clone().jobs[0].args[0]);
            assert_eq!("echo", &pipeline.clone().jobs[1].args[0]);
            assert_eq!("hello", &pipeline.clone().jobs[1].args[1]);
            assert_eq!("cat", &pipeline.clone().jobs[2].args[0]);
            assert_eq!("stuff", &pipeline.clone().stdin.unwrap().file);
            assert_eq!("other", &pipeline.clone().stdout.unwrap().file);
            assert!(!pipeline.clone().stdout.unwrap().append);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipeline_with_redirection_append() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat < stuff >> other") {
        assert_eq!(3, pipeline.jobs.len());
        assert_eq!("stuff", &pipeline.clone().stdin.unwrap().file);
        assert_eq!("other", &pipeline.clone().stdout.unwrap().file);
        assert!(pipeline.clone().stdout.unwrap().append);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn pipeline_with_redirection_reverse_order() {
        if let Statement::Pipeline(pipeline) = parse("cat | echo hello | cat > stuff < other") {
            assert_eq!(3, pipeline.jobs.len());
            assert_eq!("other", &pipeline.clone().stdin.unwrap().file);
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
}
