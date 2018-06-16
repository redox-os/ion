use super::{flow::FlowLogic, Shell, variables::VariableType};
use parser::{assignments::*, pipelines::Pipeline};
use smallvec::SmallVec;
use std::fmt::{self, Display, Formatter};
use types::Identifier;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ElseIf {
    pub expression: Pipeline,
    pub success:    Vec<Statement>,
}

/// Represents a single branch in a match statement. For example, in the expression
/// ```ignore
/// match value
///   ...
///   case value
///     statement0
///     statement1
///     ...
///     statementN
///   end
/// end
/// ```
/// would be represented by the Case object:
/// ```rust,ignore
/// Case {
///     value:      Some(value),
///     statements: vec![statement0, statement1, ... statementN],
/// }
/// ```
/// The wildcard branch, a branch that matches any value, is represented as such:
/// ```rust,ignore
/// Case { value: None, ... }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Case {
    pub value:       Option<String>,
    pub binding:     Option<String>,
    pub conditional: Option<String>,
    pub statements:  Vec<Statement>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum LocalAction {
    List,
    Assign(String, Operator, String),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum ExportAction {
    List,
    LocalExport(String),
    Assign(String, Operator, String),
}

// TODO: Enable statements and expressions to contain &str values.
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Statement {
    Let(LocalAction),
    Case(Case),
    Export(ExportAction),
    If {
        expression: Pipeline,
        success:    Vec<Statement>,
        else_if:    Vec<ElseIf>,
        failure:    Vec<Statement>,
    },
    ElseIf(ElseIf),
    Function {
        name:        Identifier,
        description: Option<String>,
        args:        Vec<KeyBuf>,
        statements:  Vec<Statement>,
    },
    For {
        variable:   Identifier,
        values:     Vec<String>,
        statements: Vec<Statement>,
    },
    While {
        expression: Pipeline,
        statements: Vec<Statement>,
    },
    Match {
        expression: String,
        cases:      Vec<Case>,
    },
    Else,
    End,
    Error(i32),
    Break,
    Continue,
    Pipeline(Pipeline),
    Time(Box<Statement>),
    And(Box<Statement>),
    Or(Box<Statement>),
    Not(Box<Statement>),
    Default,
}

impl Statement {
    pub(crate) fn short(&self) -> &'static str {
        match *self {
            Statement::Let { .. } => "Let { .. }",
            Statement::Case(_) => "Case { .. }",
            Statement::Export(_) => "Export { .. }",
            Statement::If { .. } => "If { .. }",
            Statement::ElseIf(_) => "ElseIf { .. }",
            Statement::Function { .. } => "Function { .. }",
            Statement::For { .. } => "For { .. }",
            Statement::While { .. } => "While { .. }",
            Statement::Match { .. } => "Match { .. }",
            Statement::Else => "Else",
            Statement::End => "End",
            Statement::Error(_) => "Error { .. }",
            Statement::Break => "Break",
            Statement::Continue => "Continue",
            Statement::Pipeline(_) => "Pipeline { .. }",
            Statement::Time(_) => "Time { .. }",
            Statement::And(_) => "And { .. }",
            Statement::Or(_) => "Or { .. }",
            Statement::Not(_) => "Not { .. }",
            Statement::Default => "Default",
        }
    }
}

pub(crate) struct FlowControl {
    pub level:             usize,
    pub current_statement: Statement,
    pub current_if_mode:   u8, // { 0 = SUCCESS; 1 = FAILURE }
}

impl Default for FlowControl {
    fn default() -> FlowControl {
        FlowControl {
            level:             0,
            current_statement: Statement::Default,
            current_if_mode:   0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Function {
    description: Option<String>,
    name:        Identifier,
    args:        Vec<KeyBuf>,
    statements:  Vec<Statement>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum FunctionError {
    InvalidArgumentCount,
    InvalidArgumentType(Primitive, String),
}

impl Display for FunctionError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        use self::FunctionError::*;
        match *self {
            InvalidArgumentCount => write!(fmt, "invalid number of arguments"),
            InvalidArgumentType(t, ref value) => write!(fmt, "{} is not of type {}", value, t),
        }
    }
}

impl Function {
    pub(crate) fn execute<S: AsRef<str>>(self, shell: &mut Shell, args: &[S]) -> Result<(), FunctionError> {
        if args.len() - 1 != self.args.len() {
            return Err(FunctionError::InvalidArgumentCount);
        }

        let name = self.name.clone();

        let mut values: SmallVec<[_; 8]> = SmallVec::new();

        for (type_, value) in self.args.iter().zip(args.iter().skip(1)) {
            let value = match value_check(shell, value.as_ref(), type_.kind) {
                Ok(value) => value,
                Err(_) => {
                    return Err(FunctionError::InvalidArgumentType(
                        type_.kind,
                        value.as_ref().into(),
                    ))
                }
            };

            values.push((type_.clone(), value));
        }

        let mut index = None;
        for (i, scope) in shell.variables.scopes.iter().enumerate().rev() {
            if scope.contains_key(&name) {
                index = Some(i);
                break;
            }
        }
        let index = index.expect("execute called with invalid function");

        // Pop off all scopes since function temporarily
        let temporary: Vec<_> = shell.variables.scopes.drain(index+1..).collect();
        shell.variables.new_scope();

        for (type_, value) in values {
            match value {
                ReturnValue::Vector(vector) => {
                    shell.variables.shadow(type_.name.into(), VariableType::Array(vector));
                }
                ReturnValue::Str(string) => {
                    shell.variables.shadow(type_.name.into(), VariableType::Variable(string));
                }
            }
        }

        shell.execute_statements(self.statements);

        shell.variables.pop_scope();
        shell.variables.scopes.extend(temporary);
        Ok(())
    }

    pub(crate) fn get_description<'a>(&'a self) -> Option<&'a String> { self.description.as_ref() }

    pub(crate) fn new(
        description: Option<String>,
        name: Identifier,
        args: Vec<KeyBuf>,
        statements: Vec<Statement>,
    ) -> Function {
        Function {
            description,
            name,
            args,
            statements,
        }
    }
}

pub(crate) fn collect_cases<I>(
    iterator: &mut I,
    cases: &mut Vec<Case>,
    level: &mut usize,
) -> Result<(), String>
where
    I: Iterator<Item = Statement>,
{
    macro_rules! add_to_case {
        ($statement:expr) => {
            match cases.last_mut() {
                // XXX: When does this actually happen? What syntax error is this???
                None => {
                    return Err([
                        "ion: syntax error: encountered ",
                        $statement.short(),
                        " outside of `case ...` block",
                    ].concat())
                }
                Some(ref mut case) => case.statements.push($statement),
            }
        };
    }

    while let Some(statement) = iterator.next() {
        match statement {
            Statement::Case(case) => {
                if *level == 1 {
                    // If the level is 1, then we are at a top-level case
                    // statement for this match block and should push this case
                    cases.push(case);
                } else {
                    // This is just part of the current case block
                    add_to_case!(Statement::Case(case));
                }
            }
            Statement::End => {
                *level -= 1;
                if *level == 0 {
                    return Ok(());
                }
            }
            Statement::While { .. }
            | Statement::For { .. }
            | Statement::If { .. }
            | Statement::Match { .. }
            | Statement::Function { .. } => {
                *level += 1;
                add_to_case!(statement);
            }
            Statement::Default
            | Statement::Else
            | Statement::ElseIf { .. }
            | Statement::Error(_)
            | Statement::Export(_)
            | Statement::Continue
            | Statement::Let { .. }
            | Statement::Pipeline(_)
            | Statement::Time(_)
            | Statement::And(_)
            | Statement::Or(_)
            | Statement::Not(_)
            | Statement::Break => {
                // This is the default case with all of the other statements explicitly listed
                add_to_case!(statement);
            }
        }
    }
    return Ok(());
}

pub(crate) fn collect_loops<I: Iterator<Item = Statement>>(
    iterator: &mut I,
    statements: &mut Vec<Statement>,
    level: &mut usize,
) {
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While { .. }
            | Statement::For { .. }
            | Statement::If { .. }
            | Statement::Function { .. }
            | Statement::Match { .. } => *level += 1,
            Statement::Time(ref box_stmt) => match box_stmt.as_ref() {
                &Statement::While { .. }
                | &Statement::For { .. }
                | &Statement::If { .. }
                | &Statement::Function { .. }
                | &Statement::Match { .. } => *level += 1,
                &Statement::End if *level == 1 => {
                    *level = 0;
                    break;
                }
                &Statement::End => *level -= 1,
                _ => (),
            },
            Statement::And(ref box_stmt) => match box_stmt.as_ref() {
                &Statement::While { .. }
                | &Statement::For { .. }
                | &Statement::If { .. }
                | &Statement::Function { .. }
                | &Statement::Match { .. } => *level += 1,
                &Statement::End if *level == 1 => {
                    *level = 0;
                    break;
                }
                &Statement::End => *level -= 1,
                _ => (),
            },
            Statement::Or(ref box_stmt) => match box_stmt.as_ref() {
                &Statement::While { .. }
                | &Statement::For { .. }
                | &Statement::If { .. }
                | &Statement::Function { .. }
                | &Statement::Match { .. } => *level += 1,
                &Statement::End if *level == 1 => {
                    *level = 0;
                    break;
                }
                &Statement::End => *level -= 1,
                _ => (),
            },
            Statement::Not(ref box_stmt) => match box_stmt.as_ref() {
                &Statement::While { .. }
                | &Statement::For { .. }
                | &Statement::If { .. }
                | &Statement::Function { .. }
                | &Statement::Match { .. } => *level += 1,
                &Statement::End if *level == 1 => {
                    *level = 0;
                    break;
                }
                &Statement::End => *level -= 1,
                _ => (),
            },
            Statement::End if *level == 1 => {
                *level = 0;
                break;
            }
            Statement::End => *level -= 1,
            _ => (),
        }
        statements.push(statement);
    }
}

pub(crate) fn collect_if<I>(
    iterator: &mut I,
    success: &mut Vec<Statement>,
    else_if: &mut Vec<ElseIf>,
    failure: &mut Vec<Statement>,
    level: &mut usize,
    mut current_block: u8,
) -> Result<u8, &'static str>
where
    I: Iterator<Item = Statement>,
{
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While { .. }
            | Statement::For { .. }
            | Statement::If { .. }
            | Statement::Function { .. }
            | Statement::Match { .. } => *level += 1,
            Statement::ElseIf(ref elseif) if *level == 1 => if current_block == 1 {
                return Err("ion: syntax error: else block already given");
            } else {
                current_block = 2;
                else_if.push(elseif.clone());
                continue;
            },
            Statement::Else if *level == 1 => {
                current_block = 1;
                continue;
            }
            Statement::Else if *level == 1 && current_block == 1 => {
                return Err("ion: syntax error: else block already given")
            }
            Statement::End if *level == 1 => {
                *level = 0;
                break;
            }
            Statement::End => *level -= 1,
            _ => (),
        }

        match current_block {
            0 => success.push(statement),
            1 => failure.push(statement),
            2 => {
                let last = else_if.last_mut().unwrap(); // This is a bug if there isn't a value
                last.success.push(statement);
            }
            _ => unreachable!(),
        }
    }

    Ok(current_block)
}
