use parser::peg::{Job, JobKind, Pipeline, Redirection};

const BACKSLASH:    u8 = 1;
const SINGLE_QUOTE: u8 = 2;
const DOUBLE_QUOTE: u8 = 4;
const WHITESPACE:   u8 = 8;
const PROCESS_ONE:  u8 = 64;
const PROCESS_TWO:  u8 = 128;

// Only valid if `SINGLE_QUOTE` and `DOUBLE_QUOTE` are not enabled
const PROCESS_VAL:  u8 = 255 ^ (BACKSLASH + WHITESPACE + 32);

// Determines if the character is not quoted and isn't process matched. `flags & IS_VALID` returns 0 if true
const IS_VALID: u8 = 255 ^ (BACKSLASH + WHITESPACE);

#[derive(PartialEq)]
enum RedirMode { False, Stdin, Stdout, StdoutAppend }

fn get_job_kind(args: &str, index: usize, pipe_char_was_found: bool) -> (JobKind, bool) {
    if pipe_char_was_found {
        if args.bytes().nth(index) == Some(b'|') {
            (JobKind::Or, true)
        } else {
            (JobKind::Pipe, false)
        }
    } else if args.bytes().nth(index) == Some(b'&') {
        (JobKind::And, true)
    } else {
        (JobKind::Background, false)
    }
}

/// Parses each individual pipeline, separating arguments, pipes, background tasks, and redirections.
pub fn collect(pipelines: &mut Vec<Pipeline>, possible_error: &mut Option<&str>, args: &str) {
    let mut jobs: Vec<Job> = Vec::new();
    let mut args_iter = args.bytes();
    let (mut index, mut arg_start) = (0, 0);
    let mut flags = 0u8; // (backslash, single_quote, double_quote, x, x, x, process_one, process_two)

    let mut arguments: Vec<String> = Vec::new();

    let (mut in_file, mut out_file) = (None, None);
    let mut mode = RedirMode::False;
    let mut levels = 0;

    macro_rules! redir_check {
        ($file:ident, $name:ident, $is_append:expr) => {{
            if $file.is_none() {
                if $name.is_empty() {
                    *possible_error = Some("missing standard output file argument after '>'");
                } else {
                    $file = Some(Redirection {
                        file: unsafe { String::from_utf8_unchecked($name) },
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
            ($pipe_char_was_found:expr) => {{
                // Determine which type of job will be selected based on the next character.
                //
                // - If the `|` char was found and the next character is `|`, it's `Or`
                // - If the `&` char was found and the next character is `&`, it's `And`
                let (kind, advance) = get_job_kind(args, index+1, $pipe_char_was_found);

                // If either `And` or `Or` was found, advance the iterator once.
                if advance { let _ = args_iter.next(); }

                if arguments.is_empty() {
                    jobs.push(Job::new(vec![args[arg_start..index].to_owned()], kind));
                } else {
                    if args.as_bytes()[index-1] != b' ' {
                        arguments.push(args[arg_start..index].to_owned());
                    }
                    jobs.push(Job::new(arguments.clone(), kind));
                    arguments.clear();
                }
                if advance {  index += 1; }
                arg_start = index + 1;
            }}
        }

        match mode {
            RedirMode::False => {
                while let Some(character) = args_iter.next() {
                    match character {
                        _ if flags & BACKSLASH != 0                => flags ^= BACKSLASH,
                        b'\\'                                      => flags ^= BACKSLASH,
                        b'$' if flags & PROCESS_VAL == 0           => flags |= PROCESS_ONE,
                        b'(' if flags & PROCESS_VAL == PROCESS_ONE => {
                            flags ^= PROCESS_ONE;
                            flags |= PROCESS_TWO;
                            levels += 1;
                        },
                        b')' if flags & PROCESS_VAL == PROCESS_TWO => {
                            levels -= 0;
                            if levels == 0 { flags &= 255 ^ PROCESS_TWO; }
                        },
                        b'\'' => flags ^= SINGLE_QUOTE,
                        b'"'  => flags ^= DOUBLE_QUOTE,
                        b' ' | b'\t' if (flags & IS_VALID == 0) => {
                            if arg_start != index {
                                arguments.push(args[arg_start..index].to_owned());
                                arg_start = index + 1;
                            } else {
                                arg_start += 1;
                            }
                        },
                        b'|' if (flags & (255 ^ BACKSLASH) == 0) => job_found!(true),
                        b'&' if (flags & IS_VALID == 0) => job_found!(false),
                        b'>' if (flags & IS_VALID == 0) => redir_found!(RedirMode::Stdout),
                        b'<' if (flags & IS_VALID == 0) => redir_found!(RedirMode::Stdin),
                        _   if (flags >> 6 != 2)        => flags &= 255 ^ (PROCESS_ONE + PROCESS_TWO),
                        _ => (),
                    }
                    index += 1;
                }
                break 'outer
            },
            RedirMode::Stdout | RedirMode::StdoutAppend => {
                match args_iter.next() {
                    Some(character) => if character == b'>' { mode = RedirMode::StdoutAppend; },
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
                            _ if flags & BACKSLASH != 0 => {
                                stdout_file.push(character);
                                flags ^= BACKSLASH;
                            }
                            b'\\' => flags ^= BACKSLASH,
                            b' ' | b'\t' | b'|' if stdout_file.is_empty() => (),
                            b' ' | b'\t' | b'|' => {
                                found_file = true;
                                out_file = Some(Redirection {
                                    file: unsafe { String::from_utf8_unchecked(stdout_file.clone()) },
                                    append: mode == RedirMode::StdoutAppend
                                });
                            },
                            b'<' if stdout_file.is_empty() => {
                                *possible_error = Some("missing standard output file argument after '>'");
                                break 'outer
                            }
                            b'<' => {
                                out_file = Some(Redirection {
                                    file: unsafe { String::from_utf8_unchecked(stdout_file.clone()) },
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
                let mut stdin_file = Vec::new();
                let mut found_file = false;

                while let Some(character) = args_iter.next() {
                    if found_file {
                        if character == b'>' {
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
                            b'\\' => flags ^= BACKSLASH,
                            b' ' | b'\t' | b'|' if stdin_file.is_empty() => (),
                            b' ' | b'\t' | b'|' => {
                                found_file = true;
                                in_file = Some(Redirection {
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

                redir_check!(in_file, stdin_file, false);

                break 'outer
            }
        }
    }

    if arg_start != index {
        arguments.push(args[arg_start..index].to_owned());
    }

    if !arguments.is_empty() {
        jobs.push(Job::new(arguments, JobKind::Last));
    }

    pipelines.push(Pipeline::new(jobs, in_file, out_file));
}

#[cfg(test)]
mod tests {
    use flow_control::Statement;
    use parser::peg::{parse, JobKind};

    #[test]
    fn subshells_within_subshells() {
        if let Statement::Pipelines(mut pipelines) = parse("echo $(echo one $(echo two) three)") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("echo", jobs[0].args[0]);
            assert_eq!("$(echo one $(echo two) three)", jobs[0].args[1]);
        }
    }

    #[test]
    fn quoted_process() {
        if let Statement::Pipelines(mut pipelines) = parse("let A = \"$(seq 1 10)\"") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("let", jobs[0].args[0]);
            assert_eq!("A", jobs[0].args[1]);
            assert_eq!("=", jobs[0].args[2]);
            assert_eq!("\"$(seq 1 10)\"", jobs[0].args[3]);
            assert_eq!(4, jobs[0].args.len());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn process() {
        if let Statement::Pipelines(mut pipelines) = parse("let A = $(seq 1 10)") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!("let", jobs[0].args[0]);
            assert_eq!("A", jobs[0].args[1]);
            assert_eq!("=", jobs[0].args[2]);
            assert_eq!("$(seq 1 10)", jobs[0].args[3]);
            assert_eq!(4, jobs[0].args.len());
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
    fn job_with_args() {
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
            assert_eq!(JobKind::Last, jobs[0].kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn background_job() {
        if let Statement::Pipelines(mut pipelines) = parse("echo hello world&") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(JobKind::Background, jobs[0].kind);
        } else {
            assert!(false);
        }

        if let Statement::Pipelines(mut pipelines) = parse("echo hello world &") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(JobKind::Background, jobs[0].kind);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn and_job() {
        if let Statement::Pipelines(mut pipelines) = parse("echo one && echo two") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(JobKind::And, jobs[0].kind);
            assert_eq!(vec![String::from("echo"), String::from("one")], jobs[0].args);
            assert_eq!(vec![String::from("echo"), String::from("two")], jobs[1].args);
        }
    }

    #[test]
    fn or_job() {
        if let Statement::Pipelines(mut pipelines) = parse("echo one || echo two") {
            let jobs = pipelines.remove(0).jobs;
            assert_eq!(JobKind::Or, jobs[0].kind);
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
}
