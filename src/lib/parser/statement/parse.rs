use super::{
    super::pipelines::{self, Pipeline},
    case,
    functions::{collect_arguments, parse_function},
};
use lexers::{assignment_lexer, ArgumentSplitter};
use shell::{
    flow_control::{Case, ElseIf, ExportAction, LocalAction, Statement},
    status::FAILURE,
};
use small;
use std::char;

fn collect<F>(arguments: &str, statement: F) -> Statement
where
    F: Fn(Pipeline) -> Statement,
{
    match pipelines::Collector::run(arguments) {
        Ok(pipeline) => statement(pipeline),
        Err(err) => {
            eprintln!("ion: syntax error: {}", err);
            Statement::Default
        }
    }
}

fn is_valid_name(name: &str) -> bool { !name.chars().any(|c| !(c.is_alphanumeric() || c == '_')) }

pub(crate) fn parse(code: &str) -> Statement {
    let cmd = code.trim();
    match cmd {
        "end" => return Statement::End,
        "break" => return Statement::Break,
        "continue" => return Statement::Continue,
        "for" | "match" | "case" => {
            eprintln!("ion: syntax error: incomplete control flow statement");
            return Statement::Error(FAILURE);
        }
        "let" => {
            return Statement::Let(LocalAction::List);
        }
        _ if cmd.starts_with("let ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[4..].trim_left());
            let (keys, op, values) = match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    (keys.unwrap().into(), op.unwrap(), vals.into())
                }
                None => {
                    if op.is_none() {
                        eprintln!("ion: assignment error: no operator supplied.");
                    } else {
                        eprintln!("ion: assignment error: no values supplied.")
                    }
                    return Statement::Error(FAILURE);
                }
            };

            return Statement::Let(LocalAction::Assign(keys, op, values));
        }
        "export" => {
            return Statement::Export(ExportAction::List);
        }
        _ if cmd.starts_with("export ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[7..].trim_left());
            let (keys, op, values) = match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    (keys.unwrap().into(), op.unwrap(), vals.into())
                }
                None => {
                    if keys.is_none() {
                        eprintln!("ion: assignment error: no keys supplied.")
                    } else if op.is_some() {
                        eprintln!("ion: assignment error: no values supplied.")
                    } else {
                        return Statement::Export(ExportAction::LocalExport(keys.unwrap().into()));
                    }
                    return Statement::Error(FAILURE);
                }
            };

            return Statement::Export(ExportAction::Assign(keys, op, values));
        }
        _ if cmd.starts_with("if ") => {
            return Statement::If {
                expression: vec![parse(cmd[3..].trim_left())],
                success:    Vec::new(),
                else_if:    Vec::new(),
                failure:    Vec::new(),
                mode:       0,
            }
        }
        "else" => return Statement::Else,
        _ if cmd.starts_with("else") => {
            let cmd = cmd[4..].trim_left();
            if cmd.is_empty() {
                return Statement::Else;
            } else if cmd.starts_with("if ") {
                return Statement::ElseIf(ElseIf {
                    expression: vec![parse(cmd[3..].trim_left())],
                    success:    Vec::new(),
                });
            }
        }
        _ if cmd.starts_with("while ") => {
            return collect(cmd[6..].trim_left(), |pipeline| Statement::While {
                expression: vec![Statement::Pipeline(pipeline)],
                statements: Vec::new(),
            })
        }
        _ if cmd.starts_with("for ") => {
            let mut cmd = cmd[4..].trim_left();
            let pos = match cmd.find(char::is_whitespace) {
                Some(pos) => pos,
                None => {
                    eprintln!("ion: syntax error: incorrect for loop syntax");
                    return Statement::Error(FAILURE);
                }
            };

            let variable = &cmd[..pos];
            cmd = &cmd[pos..].trim_left();

            if !cmd.starts_with("in ") {
                eprintln!("ion: syntax error: incorrect for loop syntax");
                return Statement::Error(FAILURE);
            }

            return Statement::For {
                variable:   variable.into(),
                values:     ArgumentSplitter::new(cmd[3..].trim_left())
                    .map(small::String::from)
                    .collect(),
                statements: Vec::new(),
            };
        }
        _ if cmd.starts_with("case ") => {
            let (value, binding, conditional) = match cmd[5..].trim_left() {
                "_" => (None, None, None),
                value => {
                    let (value, binding, conditional) = match case::parse_case(value) {
                        Ok(values) => values,
                        Err(why) => {
                            eprintln!("ion: case error: {}", why);
                            return Statement::Error(FAILURE);
                        }
                    };
                    let binding = binding.map(Into::into);
                    match value {
                        Some("_") => (None, binding, conditional),
                        Some(value) => (Some(value.into()), binding, conditional),
                        None => (None, binding, conditional),
                    }
                }
            };

            return Statement::Case(Case {
                value,
                binding,
                conditional,
                statements: Vec::new(),
            });
        }
        _ if cmd.starts_with("match ") => {
            return Statement::Match {
                expression: cmd[6..].trim_left().into(),
                cases:      Vec::new(),
            }
        }
        _ if cmd.starts_with("fn ") => {
            let cmd = cmd[3..].trim_left();
            let pos = cmd.find(char::is_whitespace).unwrap_or_else(|| cmd.len());
            let name = &cmd[..pos];
            if !is_valid_name(name) {
                eprintln!(
                    "ion: syntax error: '{}' is not a valid function name\n     Function names \
                     may only contain alphanumeric characters",
                    name
                );
                return Statement::Error(FAILURE);
            }

            let (args, description) = parse_function(&cmd[pos..]);
            match collect_arguments(args) {
                Ok(args) => {
                    return Statement::Function {
                        description: description.map(small::String::from),
                        name: name.into(),
                        args,
                        statements: Vec::new(),
                    }
                }
                Err(why) => {
                    eprintln!("ion: function argument error: {}", why);
                    return Statement::Error(FAILURE);
                }
            }
        }
        _ if cmd.starts_with("time ") => {
            return Statement::Time(Box::new(parse(cmd[4..].trim_left())))
        }
        _ if cmd.eq("time") => return Statement::Time(Box::new(Statement::Default)),
        _ if cmd.starts_with("and ") => {
            return Statement::And(Box::new(parse(cmd[3..].trim_left())))
        }
        _ if cmd.eq("and") => return Statement::And(Box::new(Statement::Default)),
        _ if cmd.starts_with("or ") => return Statement::Or(Box::new(parse(cmd[2..].trim_left()))),
        _ if cmd.eq("or") => return Statement::Or(Box::new(Statement::Default)),
        _ if cmd.starts_with("not ") => {
            return Statement::Not(Box::new(parse(cmd[3..].trim_left())))
        }
        _ if cmd.starts_with("! ") => return Statement::Not(Box::new(parse(cmd[1..].trim_left()))),
        _ if cmd.eq("not") | cmd.eq("!") => return Statement::Not(Box::new(Statement::Default)),
        _ => (),
    }

    if cmd.is_empty() || cmd.starts_with('#') {
        Statement::Default
    } else {
        collect(cmd, Statement::Pipeline)
    }
}

#[cfg(test)]
mod tests {
    use self::pipelines::PipeItem;
    use super::*;
    use lexers::assignments::{KeyBuf, Primitive};
    use shell::{flow_control::Statement, Job, JobKind};

    #[test]
    fn parsing_ifs() {
        // Default case where spaced normally
        let parsed_if = parse("if test 1 -eq 2");
        let correct_parse = Statement::If {
            expression: vec![Statement::Pipeline(Pipeline {
                items: vec![PipeItem {
                    job:     Job::new(
                        vec!["test".into(), "1".into(), "-eq".into(), "2".into()]
                            .into_iter()
                            .collect(),
                        JobKind::Last,
                    ),
                    outputs: Vec::new(),
                    inputs:  Vec::new(),
                }],
            })],
            success:    vec![],
            else_if:    vec![],
            failure:    vec![],
            mode:       0,
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("if test 1 -eq 2         ");
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let mut parsed_if = parse("else");
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        parsed_if = parse("else         ");
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        parsed_if = parse("         else");
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = parse("end");
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("end         ");
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         end");
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = parse("fn bob");
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        Default::default(),
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob        ");
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         fn bob");
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = parse("fn bob a b");
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        vec![
                KeyBuf {
                    name: "a".into(),
                    kind: Primitive::Any,
                },
                KeyBuf {
                    name: "b".into(),
                    kind: Primitive::Any,
                },
            ],
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob a b       ");
        assert_eq!(correct_parse, parsed_if);

        let parsed_if = parse("fn bob a b --bob is a nice function");
        let correct_parse = Statement::Function {
            description: Some("bob is a nice function".into()),
            name:        "bob".into(),
            args:        vec![
                KeyBuf {
                    name: "a".into(),
                    kind: Primitive::Any,
                },
                KeyBuf {
                    name: "b".into(),
                    kind: Primitive::Any,
                },
            ],
            statements:  vec![],
        };
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = parse("fn bob a b --          bob is a nice function");
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = parse("fn bob a b      --bob is a nice function");
        assert_eq!(correct_parse, parsed_if);
    }
}
