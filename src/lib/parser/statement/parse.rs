use super::{
    super::pipelines,
    functions::{collect_arguments, parse_function},
    Error,
};
use crate::{
    parser::lexers::{assignment_lexer, ArgumentSplitter},
    shell::{
        flow_control::{Case, ElseIf, ExportAction, IfMode, LocalAction, Statement},
        variables::Variables,
    },
    types,
};
use std::char;

pub fn parse(code: &str) -> super::Result {
    let cmd = code.trim();
    match cmd {
        "return" => Ok(Statement::Return(None)),
        _ if cmd.starts_with("return ") => {
            Ok(Statement::Return(Some(cmd[7..].trim_start().into())))
        }
        "end" => Ok(Statement::End),
        "break" => Ok(Statement::Break),
        "continue" => Ok(Statement::Continue),
        "for" | "match" | "case" => Err(Error::IncompleteFlowControl),
        "let" => Ok(Statement::Let(LocalAction::List)),
        _ if cmd.starts_with("let ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[4..].trim_start());
            match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    Ok(Statement::Let(LocalAction::Assign(
                        keys.unwrap().into(),
                        op.unwrap(),
                        vals.into(),
                    )))
                }
                None if op.is_none() => Err(Error::NoOperatorSupplied),
                _ => Err(Error::NoValueSupplied),
            }
        }
        "export" => Ok(Statement::Export(ExportAction::List)),
        _ if cmd.starts_with("export ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[7..].trim_start());
            match (vals, keys, op) {
                (Some(vals), Some(keys), Some(op)) => {
                    // If the values exist, then the keys and operator also exists.
                    Ok(Statement::Export(ExportAction::Assign(keys.into(), op, vals.into())))
                }
                (None, Some(keys), None) => {
                    Ok(Statement::Export(ExportAction::LocalExport(keys.into())))
                }
                (None, Some(_), Some(_)) => Err(Error::NoValueSupplied),
                (None, None, _) => Err(Error::NoKeySupplied),
                _ => unreachable!(),
            }
        }
        _ if cmd.starts_with("if ") => Ok(Statement::If {
            expression: vec![parse(cmd[3..].trim_start())?],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       IfMode::Success,
        }),
        "else" => Ok(Statement::Else),
        _ if cmd.starts_with("else") => {
            let cmd = cmd[4..].trim_start();
            if !cmd.is_empty() && cmd.starts_with("if ") {
                Ok(Statement::ElseIf(ElseIf {
                    expression: vec![parse(cmd[3..].trim_start())?],
                    success:    Vec::new(),
                }))
            } else {
                Ok(Statement::Else)
            }
        }
        _ if cmd.starts_with("while ") => {
            let pipeline = pipelines::Collector::run(cmd[6..].trim_start())?;
            Ok(Statement::While {
                expression: vec![Statement::Pipeline(pipeline)],
                statements: Vec::new(),
            })
        }
        _ if cmd.starts_with("for ") => {
            let cmd = cmd[4..].trim_start();
            let mut parts = cmd.splitn(2, " in ");
            let variables = parts.next().unwrap().split_whitespace().map(Into::into).collect();
            let cmd = parts.next();

            match cmd {
                Some(cmd) => Ok(Statement::For {
                    variables,
                    values: ArgumentSplitter::new(cmd.trim()).map(types::Str::from).collect(),
                    statements: Vec::new(),
                }),
                None => Err(Error::NoInKeyword),
            }
        }
        _ if cmd.starts_with("case ") => {
            Ok(Statement::Case(cmd[5..].trim_start().parse::<Case>()?))
        }
        _ if cmd.starts_with("match ") => Ok(Statement::Match {
            expression: cmd[6..].trim_start().into(),
            cases:      Vec::new(),
        }),
        _ if cmd.starts_with("fn ") => {
            let cmd = cmd[3..].trim_start();
            let pos = cmd.find(char::is_whitespace).unwrap_or_else(|| cmd.len());
            let name = &cmd[..pos];
            if !Variables::is_valid_name(name) {
                return Err(Error::InvalidFunctionName(name.into()));
            }

            let (args, description) = parse_function(&cmd[pos..]);
            Ok(Statement::Function {
                description: description.map(types::Str::from),
                name:        name.into(),
                args:        collect_arguments(args)?,
                statements:  Vec::new(),
            })
        }
        _ if cmd.starts_with("time ") => {
            // Ignore embedded time calls
            let mut timed = cmd[4..].trim_start();
            while timed.starts_with("time ") {
                timed = timed[4..].trim_start();
            }
            Ok(Statement::Time(Box::new(parse(timed)?)))
        }
        _ if cmd.eq("time") => Ok(Statement::Time(Box::new(Statement::Default))),
        _ if cmd.starts_with("and ") => Ok(Statement::And(Box::new(parse(cmd[3..].trim_start())?))),
        _ if cmd.eq("and") => Ok(Statement::And(Box::new(Statement::Default))),
        _ if cmd.starts_with("or ") => Ok(Statement::Or(Box::new(parse(cmd[2..].trim_start())?))),
        _ if cmd.eq("or") => Ok(Statement::Or(Box::new(Statement::Default))),
        _ if cmd.starts_with("not ") => Ok(Statement::Not(Box::new(parse(cmd[3..].trim_start())?))),
        _ if cmd.starts_with("! ") => Ok(Statement::Not(Box::new(parse(cmd[1..].trim_start())?))),
        _ if cmd.eq("not") | cmd.eq("!") => Ok(Statement::Not(Box::new(Statement::Default))),
        _ if cmd.is_empty() || cmd.starts_with('#') => Ok(Statement::Default),
        _ => Ok(Statement::Pipeline(pipelines::Collector::run(cmd)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expansion::pipelines::{PipeItem, PipeType, Pipeline, RedirectFrom},
        parser::lexers::{
            assignments::{KeyBuf, Primitive},
            Operator,
        },
        shell::{flow_control::Statement, Job},
    };

    #[test]
    fn parsing_let_alone() {
        assert_eq!(parse("let").unwrap(), Statement::Let(LocalAction::List),);
    }

    #[test]
    fn parsing_let() {
        assert_eq!(
            parse("let FOO = BAR").unwrap(),
            Statement::Let(LocalAction::Assign(
                "FOO".to_owned(),
                Operator::Equal,
                "BAR".to_owned()
            )),
        );
        assert_eq!(
            parse("let FooX =\"$BAR + $HI\"").unwrap(),
            Statement::Let(LocalAction::Assign(
                "FooX".to_owned(),
                Operator::Equal,
                "\"$BAR + $HI\"".to_owned()
            )),
        );
        assert_eq!(
            parse("let list = [hello world]").unwrap(),
            Statement::Let(LocalAction::Assign(
                "list".to_owned(),
                Operator::Equal,
                "[hello world]".to_owned()
            )),
        );
        // Can it handle line breaks in value ?
        assert_eq!(
            parse("let list = [hello \n world]").unwrap(),
            Statement::Let(LocalAction::Assign(
                "list".to_owned(),
                Operator::Equal,
                "[hello \n world]".to_owned()
            )),
        );
        // Can it additional spaces, new lines tabs ?
        assert_eq!(
            parse("let \t  list \n =     \n $(echo world)  ").unwrap(),
            Statement::Let(LocalAction::Assign(
                "list".to_owned(),
                Operator::Equal,
                "$(echo world)".to_owned()
            )),
        );
    }

    #[test]
    fn parsing_let_error() {
        let actual = parse("let Foo");
        assert!(
            matches!(actual, Err(Error::NoOperatorSupplied)),
            "Should return error {:?}",
            Error::NoOperatorSupplied
        );
        let actual = parse("let hello =");
        assert!(
            matches!(actual, Err(Error::NoValueSupplied)),
            "Should return error {:?}",
            Error::NoValueSupplied
        );
    }

    #[test]
    fn parsing_for() {
        assert_eq!(
            parse("for x y z in 1..=10").unwrap(),
            Statement::For {
                variables:  vec!["x", "y", "z"].into_iter().map(Into::into).collect(),
                values:     vec!["1..=10"].into_iter().map(Into::into).collect(),
                statements: Vec::new(),
            }
        );

        assert_eq!(
            parse("for  x  in  {1..=10} {1..=10}").unwrap(),
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
        let parsed_if = parse("if test 1 -eq 2").unwrap();
        let correct_parse = Statement::If {
            expression: vec![Statement::Pipeline(Pipeline {
                items: vec![PipeItem {
                    job:     Job::new(
                        vec!["test".into(), "1".into(), "-eq".into(), "2".into()]
                            .into_iter()
                            .collect(),
                        RedirectFrom::None,
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
        let parsed_if = parse("if test 1 -eq 2         ").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let mut parsed_if = parse("else").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        parsed_if = parse("else         ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        parsed_if = parse("         else").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = parse("end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("end         ").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = parse("fn bob").unwrap();
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        Vec::default(),
            statements:  Vec::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob        ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         fn bob").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = parse("fn bob a b").unwrap();
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        vec![
                KeyBuf { name: "a".into(), kind: Primitive::Str },
                KeyBuf { name: "b".into(), kind: Primitive::Str },
            ],
            statements:  Vec::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob a b       ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        let parsed_if = parse("fn bob a b --bob is a nice function").unwrap();
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
        let parsed_if = parse("fn bob a b --          bob is a nice function").unwrap();
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = parse("fn bob a b      --bob is a nice function").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }
}
