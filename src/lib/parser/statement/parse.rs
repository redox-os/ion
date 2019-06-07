use super::{
    super::pipelines,
    case,
    functions::{collect_arguments, parse_function},
};
use crate::{
    builtins::BuiltinMap,
    lexers::{assignment_lexer, ArgumentSplitter},
    shell::{
        flow_control::{Case, ElseIf, ExportAction, IfMode, LocalAction, Statement},
        status::Status,
    },
};
use small;
use std::char;

pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().map_or(false, |b| char::is_alphabetic(b) || b == '_')
        && chars.all(|b| b.is_alphanumeric() || b == '_')
}

pub fn parse<'a>(code: &str, builtins: &BuiltinMap<'a>) -> Statement<'a> {
    let cmd = code.trim();
    match cmd {
        "end" => Statement::End,
        "break" => Statement::Break,
        "continue" => Statement::Continue,
        "for" | "match" | "case" => {
            Statement::Error(Status::error("ion: syntax error: incomplete control flow statement"))
        }
        "let" => Statement::Let(LocalAction::List),
        _ if cmd.starts_with("let ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[4..].trim_start());
            match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    Statement::Let(LocalAction::Assign(
                        keys.unwrap().into(),
                        op.unwrap(),
                        vals.into(),
                    ))
                }
                None => {
                    if op.is_none() {
                        Statement::Error(Status::error(
                            "ion: assignment error: no operator supplied.",
                        ))
                    } else {
                        Statement::Error(Status::error(
                            "ion: assignment error: no values supplied.",
                        ))
                    }
                }
            }
        }
        "export" => Statement::Export(ExportAction::List),
        _ if cmd.starts_with("export ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[7..].trim_start());
            match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    Statement::Export(ExportAction::Assign(
                        keys.unwrap().into(),
                        op.unwrap(),
                        vals.into(),
                    ))
                }
                None => {
                    if keys.is_none() {
                        Statement::Error(Status::error("ion: assignment error: no keys supplied."))
                    } else if op.is_some() {
                        Statement::Error(Status::error(
                            "ion: assignment error: no values supplied.",
                        ))
                    } else {
                        Statement::Export(ExportAction::LocalExport(keys.unwrap().into()))
                    }
                }
            }
        }
        _ if cmd.starts_with("if ") => Statement::If {
            expression: vec![parse(cmd[3..].trim_start(), builtins)],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       IfMode::Success,
        },
        "else" => Statement::Else,
        _ if cmd.starts_with("else") => {
            let cmd = cmd[4..].trim_start();
            if !cmd.is_empty() && cmd.starts_with("if ") {
                Statement::ElseIf(ElseIf {
                    expression: vec![parse(cmd[3..].trim_start(), builtins)],
                    success:    Vec::new(),
                })
            } else {
                Statement::Else
            }
        }
        _ if cmd.starts_with("while ") => {
            match pipelines::Collector::run(cmd[6..].trim_start(), builtins) {
                Ok(pipeline) => Statement::While {
                    expression: vec![Statement::Pipeline(pipeline)],
                    statements: Vec::new(),
                },
                Err(err) => {
                    eprintln!("ion: syntax error: {}", err);
                    Statement::Default
                }
            }
        }
        _ if cmd.starts_with("for ") => {
            let mut cmd = cmd[4..].trim_start();
            let mut variables = None;

            if cmd.len() > 5 {
                let cmdb = cmd.as_bytes();
                for start in 0..cmd.len() - 4 {
                    if &cmdb[start..start + 4] == b" in " {
                        variables = Some(cmd[..start].split_whitespace().map(Into::into).collect());

                        cmd = cmd[start + 3..].trim();

                        break;
                    }
                }
            }

            match variables {
                Some(variables) => Statement::For {
                    variables,
                    values: ArgumentSplitter::new(cmd).map(small::String::from).collect(),
                    statements: Vec::new(),
                },
                None => Statement::Error(Status::error(
                    "ion: syntax error: for loop lacks the `in` keyword",
                )),
            }
        }
        _ if cmd.starts_with("case ") => {
            let (value, binding, conditional) = match cmd[5..].trim_start() {
                "_" => (None, None, None),
                value => {
                    let (value, binding, conditional) = match case::parse_case(value) {
                        Ok(values) => values,
                        Err(why) => {
                            return Statement::Error(Status::error(format!(
                                "ion: case error: {}",
                                why
                            )))
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

            Statement::Case(Case { value, binding, conditional, statements: Vec::new() })
        }
        _ if cmd.starts_with("match ") => {
            Statement::Match { expression: cmd[6..].trim_start().into(), cases: Vec::new() }
        }
        _ if cmd.starts_with("fn ") => {
            let cmd = cmd[3..].trim_start();
            let pos = cmd.find(char::is_whitespace).unwrap_or_else(|| cmd.len());
            let name = &cmd[..pos];
            if !is_valid_name(name) {
                return Statement::Error(Status::error(format!(
                    "ion: syntax error: '{}' is not a valid function name\n     Function names \
                     may only contain alphanumeric characters",
                    name
                )));
            }

            let (args, description) = parse_function(&cmd[pos..]);
            match collect_arguments(args) {
                Ok(args) => Statement::Function {
                    description: description.map(small::String::from),
                    name: name.into(),
                    args,
                    statements: Vec::new(),
                },
                Err(why) => Statement::Error(Status::error(format!(
                    "ion: function argument error: {}",
                    why
                ))),
            }
        }
        _ if cmd.starts_with("time ") => {
            // Ignore embedded time calls
            let mut timed = cmd[4..].trim_start();
            while timed.starts_with("time ") {
                timed = timed[4..].trim_start();
            }
            Statement::Time(Box::new(parse(timed, builtins)))
        }
        _ if cmd.eq("time") => Statement::Time(Box::new(Statement::Default)),
        _ if cmd.starts_with("and ") => {
            Statement::And(Box::new(parse(cmd[3..].trim_start(), builtins)))
        }
        _ if cmd.eq("and") => Statement::And(Box::new(Statement::Default)),
        _ if cmd.starts_with("or ") => {
            Statement::Or(Box::new(parse(cmd[2..].trim_start(), builtins)))
        }
        _ if cmd.eq("or") => Statement::Or(Box::new(Statement::Default)),
        _ if cmd.starts_with("not ") => {
            Statement::Not(Box::new(parse(cmd[3..].trim_start(), builtins)))
        }
        _ if cmd.starts_with("! ") => {
            Statement::Not(Box::new(parse(cmd[1..].trim_start(), builtins)))
        }
        _ if cmd.eq("not") | cmd.eq("!") => Statement::Not(Box::new(Statement::Default)),
        _ if cmd.is_empty() || cmd.starts_with('#') => Statement::Default,
        _ => match pipelines::Collector::run(cmd, builtins) {
            Ok(pipeline) => Statement::Pipeline(pipeline),
            Err(err) => {
                eprintln!("ion: syntax error: {}", err);
                Statement::Default
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use self::pipelines::{PipeItem, PipeType, Pipeline};
    use super::*;
    use crate::{
        builtins::BuiltinMap,
        lexers::assignments::{KeyBuf, Primitive},
        parser::pipelines::RedirectFrom,
        shell::{flow_control::Statement, Job},
    };

    #[test]
    fn parsing_for() {
        assert_eq!(
            parse("for x y z in 1..=10", &BuiltinMap::new()),
            Statement::For {
                variables:  vec!["x", "y", "z"].into_iter().map(Into::into).collect(),
                values:     vec!["1..=10"].into_iter().map(Into::into).collect(),
                statements: Vec::new(),
            }
        );

        assert_eq!(
            parse("for  x  in  {1..=10} {1..=10}", &BuiltinMap::new()),
            Statement::For {
                variables:  vec!["x"].into_iter().map(Into::into).collect(),
                values:     vec!["{1..=10}", "{1..=10}"].into_iter().map(Into::into).collect(),
                statements: Vec::new(),
            }
        );
    }

    #[test]
    fn parsing_ifs() {
        // Default case where spaced normally
        let parsed_if = parse("if test 1 -eq 2", &BuiltinMap::new());
        let correct_parse = Statement::If {
            expression: vec![Statement::Pipeline(Pipeline {
                items: vec![PipeItem {
                    job:     Job::new(
                        vec!["test".into(), "1".into(), "-eq".into(), "2".into()]
                            .into_iter()
                            .collect(),
                        RedirectFrom::None,
                        None,
                    ),
                    outputs: Vec::new(),
                    inputs:  Vec::new(),
                }],
                pipe:  PipeType::Normal,
            })],
            success:    vec![],
            else_if:    vec![],
            failure:    vec![],
            mode:       IfMode::Success,
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("if test 1 -eq 2         ", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let mut parsed_if = parse("else", &BuiltinMap::new());
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        parsed_if = parse("else         ", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        parsed_if = parse("         else", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = parse("end", &BuiltinMap::new());
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("end         ", &BuiltinMap::new());
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         end", &BuiltinMap::new());
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = parse("fn bob", &BuiltinMap::new());
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        Default::default(),
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob        ", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         fn bob", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = parse("fn bob a b", &BuiltinMap::new());
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        vec![
                KeyBuf { name: "a".into(), kind: Primitive::Str },
                KeyBuf { name: "b".into(), kind: Primitive::Str },
            ],
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob a b       ", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);

        let parsed_if = parse("fn bob a b --bob is a nice function", &BuiltinMap::new());
        let correct_parse = Statement::Function {
            description: Some("bob is a nice function".into()),
            name:        "bob".into(),
            args:        vec![
                KeyBuf { name: "a".into(), kind: Primitive::Str },
                KeyBuf { name: "b".into(), kind: Primitive::Str },
            ],
            statements:  vec![],
        };
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = parse("fn bob a b --          bob is a nice function", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = parse("fn bob a b      --bob is a nice function", &BuiltinMap::new());
        assert_eq!(correct_parse, parsed_if);
    }
}
