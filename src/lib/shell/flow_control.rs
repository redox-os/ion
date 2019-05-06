use crate::{
    lexers::assignments::{KeyBuf, Operator, Primitive},
    parser::{assignments::*, pipelines::Pipeline},
    shell::{flow::FlowLogic, Shell},
    types,
};
use small;
use smallvec::SmallVec;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ElseIf<'a> {
    pub expression: Vec<Statement<'a>>,
    pub success:    Vec<Statement<'a>>,
}

/// Represents a single branch in a match statement. For example, in the expression
/// ```ignore
/// match value
///   ...
///   case not_value
///     statement0
///     statement1
///     ...
///     statementN
///   case value
///     statement0
///     statement1
///     ...
///     statementM
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
pub(crate) struct Case<'a> {
    pub value:       Option<String>,
    pub binding:     Option<String>,
    pub conditional: Option<String>,
    pub statements:  Vec<Statement<'a>>,
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
pub(crate) enum Statement<'a> {
    Let(LocalAction),
    Case(Case<'a>),
    Export(ExportAction),
    If {
        expression: Vec<Statement<'a>>,
        success:    Vec<Statement<'a>>,
        else_if:    Vec<ElseIf<'a>>,
        failure:    Vec<Statement<'a>>,
        mode:       u8, // {0 = success, 1 = else_if, 2 = failure}
    },
    ElseIf(ElseIf<'a>),
    Function {
        name:        types::Str,
        description: Option<small::String>,
        args:        Vec<KeyBuf>,
        statements:  Vec<Statement<'a>>,
    },
    For {
        variables:  SmallVec<[types::Str; 4]>,
        values:     Vec<small::String>,
        statements: Vec<Statement<'a>>,
    },
    While {
        expression: Vec<Statement<'a>>,
        statements: Vec<Statement<'a>>,
    },
    Match {
        expression: small::String,
        cases:      Vec<Case<'a>>,
    },
    Else,
    End,
    Error(i32),
    Break,
    Continue,
    Pipeline(Pipeline<'a>),
    Time(Box<Statement<'a>>),
    And(Box<Statement<'a>>),
    Or(Box<Statement<'a>>),
    Not(Box<Statement<'a>>),
    Default,
}

impl<'a> Statement<'a> {
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

    pub fn is_block(&self) -> bool {
        match *self {
            Statement::Case(_)
            | Statement::If { .. }
            | Statement::ElseIf(_)
            | Statement::Function { .. }
            | Statement::For { .. }
            | Statement::While { .. }
            | Statement::Match { .. }
            | Statement::Else => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FlowControl<'a> {
    pub block: Vec<Statement<'a>>,
}

impl<'a> FlowControl<'a> {
    /// On error reset FlowControl fields.
    pub(crate) fn reset(&mut self) { self.block.clear() }

    /// Discard one block.
    pub(crate) fn pop(&mut self) -> bool { self.block.pop().is_some() }

    /// Check if there isn't an unfinished block.
    pub(crate) fn unclosed_block(&self) -> Option<&str> {
        self.block.last().map(|block| block.short())
    }
}

impl<'a> Default for FlowControl<'a> {
    fn default() -> Self { FlowControl { block: Vec::with_capacity(5) } }
}

pub(crate) fn insert_statement<'a>(
    flow_control: &mut FlowControl<'a>,
    statement: Statement<'a>,
) -> Result<Option<Statement<'a>>, &'static str> {
    match statement {
        // Push new block to stack
        Statement::For { .. }
        | Statement::While { .. }
        | Statement::Match { .. }
        | Statement::If { .. }
        | Statement::Function { .. } => {
            flow_control.block.push(statement);
            Ok(None)
        }
        // Case is special as it should pop back previous Case
        Statement::Case(_) => {
            match flow_control.block.last() {
                Some(Statement::Case(_)) => {
                    let case = flow_control.block.pop().unwrap();
                    let _ = insert_into_block(&mut flow_control.block, case);
                }
                Some(Statement::Match { .. }) => (),
                _ => return Err("ion: error: Case { .. } found outside of Match { .. } block"),
            }

            flow_control.block.push(statement);
            Ok(None)
        }
        Statement::End => {
            match flow_control.block.len() {
                0 => Err("ion: error: keyword End found but no block to close"),
                // Ready to return the complete block
                1 => Ok(flow_control.block.pop()),
                // Merge back the top block into the previous one
                _ => {
                    let block = flow_control.block.pop().unwrap();
                    let mut case = false;
                    if let Statement::Case(_) = block {
                        case = true;
                    }
                    insert_into_block(&mut flow_control.block, block)?;
                    if case {
                        // Merge last Case back and pop off Match too
                        let match_stm = flow_control.block.pop().unwrap();
                        if !flow_control.block.is_empty() {
                            insert_into_block(&mut flow_control.block, match_stm)?;

                            Ok(None)
                        } else {
                            Ok(Some(match_stm))
                        }
                    } else {
                        Ok(None)
                    }
                }
            }
        }
        Statement::And(_) | Statement::Or(_) if !flow_control.block.is_empty() => {
            let mut pushed = true;
            match flow_control.block.last_mut().unwrap() {
                Statement::If {
                    ref mut expression,
                    ref mode,
                    ref success,
                    ref mut else_if,
                    ..
                } => match *mode {
                    0 if success.is_empty() => {
                        // Insert into If expression if there's no previous statement.
                        expression.push(statement.clone());
                    }
                    1 => {
                        // Try to insert into last ElseIf expression if there's no previous
                        // statement.
                        if let Some(eif) = else_if.last_mut() {
                            if eif.success.is_empty() {
                                eif.expression.push(statement.clone());
                            } else {
                                pushed = false;
                            }
                        } else {
                            // should not be reached...
                            unreachable!("Missmatch in 'If' mode!")
                        }
                    }
                    _ => pushed = false,
                },
                Statement::While { ref mut expression, ref statements } => {
                    if statements.is_empty() {
                        expression.push(statement.clone());
                    } else {
                        pushed = false;
                    }
                }
                _ => pushed = false,
            }
            if !pushed {
                insert_into_block(&mut flow_control.block, statement)?;
            }

            Ok(None)
        }
        Statement::Time(inner) => {
            if inner.is_block() {
                flow_control.block.push(Statement::Time(inner));
                Ok(None)
            } else {
                Ok(Some(Statement::Time(inner)))
            }
        }
        _ => {
            if !flow_control.block.is_empty() {
                insert_into_block(&mut flow_control.block, statement)?;
                Ok(None)
            } else {
                // Filter out toplevel statements that should produce an error
                // otherwise return the statement for immediat execution
                match statement {
                    Statement::ElseIf(_) => {
                        Err("ion: error: found ElseIf { .. } without If { .. } block")
                    }
                    Statement::Else => Err("ion: error: found Else without If { .. } block"),
                    Statement::Break => Err("ion: error: found Break without loop body"),
                    Statement::Continue => Err("ion: error: found Continue without loop body"),
                    // Toplevel statement, return to execute immediately
                    _ => Ok(Some(statement)),
                }
            }
        }
    }
}

fn insert_into_block<'a>(
    block: &mut Vec<Statement<'a>>,
    statement: Statement<'a>,
) -> Result<(), &'static str> {
    let block = match block.last_mut().expect("Should not insert statement if stack is empty!") {
        Statement::Time(inner) => inner,
        top_block => top_block,
    };

    match block {
        Statement::Function { ref mut statements, .. } => statements.push(statement),
        Statement::For { ref mut statements, .. } => statements.push(statement),
        Statement::While { ref mut statements, .. } => statements.push(statement),
        Statement::Match { ref mut cases, .. } => match statement {
            Statement::Case(case) => cases.push(case),
            _ => {
                return Err(
                    "ion: error: statement found outside of Case { .. } block in Match { .. }"
                );
            }
        },
        Statement::Case(ref mut case) => case.statements.push(statement),
        Statement::If {
            ref mut success, ref mut else_if, ref mut failure, ref mut mode, ..
        } => match statement {
            Statement::ElseIf(eif) => {
                if *mode > 1 {
                    return Err("ion: error: ElseIf { .. } found after Else");
                } else {
                    *mode = 1;
                    else_if.push(eif);
                }
            }
            Statement::Else => {
                if *mode == 2 {
                    return Err("ion: error: Else block already exists");
                } else {
                    *mode = 2;
                }
            }
            _ => match *mode {
                0 => success.push(statement),
                1 => else_if.last_mut().unwrap().success.push(statement),
                2 => failure.push(statement),
                _ => unreachable!(),
            },
        },
        _ => unreachable!("Not block-like statement pushed to stack!"),
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function<'a> {
    description: Option<small::String>,
    name:        types::Str,
    args:        Vec<KeyBuf>,
    statements:  Vec<Statement<'a>>,
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
            InvalidArgumentType(ref t, ref value) => write!(fmt, "{} is not of type {}", value, t),
        }
    }
}

impl<'a> Function<'a> {
    pub fn is_empty(&self) -> bool { self.statements.is_empty() }

    pub(crate) fn execute<S: AsRef<str>>(
        &self,
        shell: &mut Shell<'a>,
        args: &[S],
    ) -> Result<(), FunctionError> {
        if args.len() - 1 != self.args.len() {
            return Err(FunctionError::InvalidArgumentCount);
        }

        let values = self
            .args
            .iter()
            .zip(args.iter().skip(1))
            .map(|(type_, value)| {
                if let Ok(value) = value_check(shell, value.as_ref(), &type_.kind) {
                    Ok((type_.clone(), value))
                } else {
                    Err(FunctionError::InvalidArgumentType(
                        type_.kind.clone(),
                        value.as_ref().into(),
                    ))
                }
            })
            .collect::<Result<SmallVec<[_; 8]>, _>>()?;

        let index = shell
            .variables
            .index_scope_for_var(&self.name)
            .expect("execute called with invalid function");

        // Pop off all scopes since function temporarily
        let temporary: Vec<_> = shell.variables.pop_scopes(index).collect();

        shell.variables.new_scope(true);

        for (type_, value) in values {
            shell.variables.shadow(&type_.name, value);
        }

        shell.execute_statements(&self.statements);

        shell.variables.pop_scope();
        shell.variables.append_scopes(temporary);
        Ok(())
    }

    pub(crate) fn get_description(&self) -> Option<&small::String> { self.description.as_ref() }

    pub(crate) fn new(
        description: Option<small::String>,
        name: types::Str,
        args: Vec<KeyBuf>,
        statements: Vec<Statement>,
    ) -> Function {
        Function { description, name, args, statements }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_match() -> Statement<'static> {
        Statement::Match { expression: small::String::from(""), cases: Vec::new() }
    }
    fn new_if() -> Statement<'static> {
        Statement::If {
            expression: vec![Statement::Default],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       0,
        }
    }
    fn new_case() -> Statement<'static> {
        Statement::Case(Case {
            value:       None,
            binding:     None,
            conditional: None,
            statements:  Vec::new(),
        })
    }

    #[test]
    fn if_inside_match() {
        let mut flow_control = FlowControl::default();

        let res = insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.block.len(), 1);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.block.len(), 2);
        assert_eq!(res, Ok(None));

        // Pops back top case, len stays 2
        let res = insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.block.len(), 2);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, new_if());
        assert_eq!(flow_control.block.len(), 3);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.block.len(), 2);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.block.len(), 0);
        if let Ok(Some(Statement::Match { ref cases, .. })) = res {
            assert_eq!(cases.len(), 2);
            assert_eq!(cases.last().unwrap().statements.len(), 1);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn statement_outside_case() {
        let mut flow_control = FlowControl::default();

        let res = insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.block.len(), 1);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::Default);
        if res.is_err() {
            flow_control.reset();
            assert_eq!(flow_control.block.len(), 0);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn return_toplevel() {
        let mut flow_control = FlowControl::default();
        let oks = vec![
            Statement::Error(1),
            Statement::Time(Box::new(Statement::Default)),
            Statement::And(Box::new(Statement::Default)),
            Statement::Or(Box::new(Statement::Default)),
            Statement::Not(Box::new(Statement::Default)),
            Statement::Default,
        ];
        for ok in oks {
            let res = insert_statement(&mut flow_control, ok.clone());
            assert_eq!(Ok(Some(ok)), res);
        }

        let errs = vec![Statement::Else, Statement::End, Statement::Break, Statement::Continue];
        for err in errs {
            let res = insert_statement(&mut flow_control, err);
            if res.is_ok() {
                assert!(false);
            }
        }
    }
}
