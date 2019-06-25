use super::{
    super::pipelines,
    case,
    functions::{collect_arguments, parse_function},
};
use crate::{
    builtins::BuiltinMap,
    parser::{
        lexers::{assignment_lexer, ArgumentSplitter},
        pipelines::PipelineParsingError,
        statement::{case::CaseError, functions::FunctionParseError},
    },
    shell::flow_control::{Case, ElseIf, ExportAction, IfMode, LocalAction, Statement},
    types,
};
use err_derive::Error;
use std::char;

/// Check if the given name is valid for functions, aliases & variables
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().map_or(false, |b| char::is_alphabetic(b) || b == '_')
        && chars.all(|b| b.is_alphanumeric() || b == '_')
}

/// An Error occured during parsing
#[derive(Debug, Error)]
pub enum ParseError {
    /// The blocks were in a wrong order
    #[error(display = "incomplete control flow statement")]
    IncompleteFlowControl,
    /// No keys were supplied for assignment
    #[error(display = "no key supplied for assignment")]
    NoKeySupplied,
    /// No operator was supplied for assignment
    #[error(display = "no operator supplied for assignment")]
    NoOperatorSupplied,
    /// No value supplied for assignment
    #[error(display = "no values supplied for assignment")]
    NoValueSupplied,
    /// No value given for iteration in a for loop
    #[error(display = "no value supplied for iteration in for loop")]
    NoInKeyword,
    /// Error with match statements
    #[error(display = "case error: {}", _0)]
    CaseError(#[error(cause)] CaseError),
    /// The provided function name was invalid
    #[error(
        display = "'{}' is not a valid function name
        Function names may only contain alphanumeric characters",
        _0
    )]
    InvalidFunctionName(String),
    /// The arguments did not match the function's signature
    #[error(display = "function argument error: {}", _0)]
    InvalidFunctionArgument(#[error(cause)] FunctionParseError),
    /// Error occured during parsing of a pipeline
    #[error(display = "{}", _0)]
    PipelineParsingError(#[error(cause)] PipelineParsingError),
}

impl From<FunctionParseError> for ParseError {
    fn from(cause: FunctionParseError) -> Self { ParseError::InvalidFunctionArgument(cause) }
}

impl From<CaseError> for ParseError {
    fn from(cause: CaseError) -> Self { ParseError::CaseError(cause) }
}

impl From<PipelineParsingError> for ParseError {
    fn from(cause: PipelineParsingError) -> Self { ParseError::PipelineParsingError(cause) }
}

pub fn parse<'a>(code: &str, builtins: &BuiltinMap<'a>) -> super::Result<'a> {
    let cmd = code.trim();
    match cmd {
        "end" => Ok(Statement::End),
        "break" => Ok(Statement::Break),
        "continue" => Ok(Statement::Continue),
        "for" | "match" | "case" => Err(ParseError::IncompleteFlowControl),
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
                None if op.is_none() => Err(ParseError::NoOperatorSupplied),
                _ => Err(ParseError::NoValueSupplied),
            }
        }
        "export" => Ok(Statement::Export(ExportAction::List)),
        _ if cmd.starts_with("export ") => {
            // Split the let expression and ensure that the statement is valid.
            let (keys, op, vals) = assignment_lexer(cmd[7..].trim_start());
            match vals {
                Some(vals) => {
                    // If the values exist, then the keys and operator also exists.
                    Ok(Statement::Export(ExportAction::Assign(
                        keys.unwrap().into(),
                        op.unwrap(),
                        vals.into(),
                    )))
                }
                None => {
                    if keys.is_none() {
                        Err(ParseError::NoKeySupplied)
                    } else if op.is_some() {
                        Err(ParseError::NoValueSupplied)
                    } else {
                        Ok(Statement::Export(ExportAction::LocalExport(keys.unwrap().into())))
                    }
                }
            }
        }
        _ if cmd.starts_with("if ") => Ok(Statement::If {
            expression: vec![parse(cmd[3..].trim_start(), builtins)?],
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
                    expression: vec![parse(cmd[3..].trim_start(), builtins)?],
                    success:    Vec::new(),
                }))
            } else {
                Ok(Statement::Else)
            }
        }
        _ if cmd.starts_with("while ") => {
            let pipeline = pipelines::Collector::run(cmd[6..].trim_start(), builtins)?;
            Ok(Statement::While {
                expression: vec![Statement::Pipeline(pipeline)],
                statements: Vec::new(),
            })
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
                Some(variables) => Ok(Statement::For {
                    variables,
                    values: ArgumentSplitter::new(cmd).map(types::Str::from).collect(),
                    statements: Vec::new(),
                }),
                None => Err(ParseError::NoInKeyword),
            }
        }
        _ if cmd.starts_with("case ") => {
            let (value, binding, conditional) = match cmd[5..].trim_start() {
                "_" => (None, None, None),
                value => {
                    let (value, binding, conditional) = case::parse_case(value)?;
                    let binding = binding.map(Into::into);
                    match value {
                        Some("_") => (None, binding, conditional),
                        Some(value) => (Some(value.into()), binding, conditional),
                        None => (None, binding, conditional),
                    }
                }
            };

            Ok(Statement::Case(Case { value, binding, conditional, statements: Vec::new() }))
        }
        _ if cmd.starts_with("match ") => Ok(Statement::Match {
            expression: cmd[6..].trim_start().into(),
            cases:      Vec::new(),
        }),
        _ if cmd.starts_with("fn ") => {
            let cmd = cmd[3..].trim_start();
            let pos = cmd.find(char::is_whitespace).unwrap_or_else(|| cmd.len());
            let name = &cmd[..pos];
            if !is_valid_name(name) {
                return Err(ParseError::InvalidFunctionName(name.into()));
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
            Ok(Statement::Time(Box::new(parse(timed, builtins)?)))
        }
        _ if cmd.eq("time") => Ok(Statement::Time(Box::new(Statement::Default))),
        _ if cmd.starts_with("and ") => {
            Ok(Statement::And(Box::new(parse(cmd[3..].trim_start(), builtins)?)))
        }
        _ if cmd.eq("and") => Ok(Statement::And(Box::new(Statement::Default))),
        _ if cmd.starts_with("or ") => {
            Ok(Statement::Or(Box::new(parse(cmd[2..].trim_start(), builtins)?)))
        }
        _ if cmd.eq("or") => Ok(Statement::Or(Box::new(Statement::Default))),
        _ if cmd.starts_with("not ") => {
            Ok(Statement::Not(Box::new(parse(cmd[3..].trim_start(), builtins)?)))
        }
        _ if cmd.starts_with("! ") => {
            Ok(Statement::Not(Box::new(parse(cmd[1..].trim_start(), builtins)?)))
        }
        _ if cmd.eq("not") | cmd.eq("!") => Ok(Statement::Not(Box::new(Statement::Default))),
        _ if cmd.is_empty() || cmd.starts_with('#') => Ok(Statement::Default),
        _ => Ok(Statement::Pipeline(pipelines::Collector::run(cmd, builtins)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builtins::BuiltinMap,
        expansion::pipelines::{PipeItem, PipeType, Pipeline, RedirectFrom},
        parser::lexers::assignments::{KeyBuf, Primitive},
        shell::{flow_control::Statement, Job},
    };

    #[test]
    fn parsing_for() {
        assert_eq!(
            parse("for x y z in 1..=10", &BuiltinMap::new()).unwrap(),
            Statement::For {
                variables:  vec!["x", "y", "z"].into_iter().map(Into::into).collect(),
                values:     vec!["1..=10"].into_iter().map(Into::into).collect(),
                statements: Vec::new(),
            }
        );

        assert_eq!(
            parse("for  x  in  {1..=10} {1..=10}", &BuiltinMap::new()).unwrap(),
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
        let parsed_if = parse("if test 1 -eq 2", &BuiltinMap::new()).unwrap();
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
        let parsed_if = parse("if test 1 -eq 2         ", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let mut parsed_if = parse("else", &BuiltinMap::new()).unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        parsed_if = parse("else         ", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        parsed_if = parse("         else", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = parse("end", &BuiltinMap::new()).unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("end         ", &BuiltinMap::new()).unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         end", &BuiltinMap::new()).unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = parse("fn bob", &BuiltinMap::new()).unwrap();
        let correct_parse = Statement::Function {
            description: None,
            name:        "bob".into(),
            args:        Default::default(),
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = parse("fn bob        ", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = parse("         fn bob", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = parse("fn bob a b", &BuiltinMap::new()).unwrap();
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
        let parsed_if = parse("fn bob a b       ", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);

        let parsed_if = parse("fn bob a b --bob is a nice function", &BuiltinMap::new()).unwrap();
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
        let parsed_if =
            parse("fn bob a b --          bob is a nice function", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);
        let parsed_if =
            parse("fn bob a b      --bob is a nice function", &BuiltinMap::new()).unwrap();
        assert_eq!(correct_parse, parsed_if);
    }
}
