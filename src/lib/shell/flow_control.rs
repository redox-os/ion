use crate::{
    assignments::*,
    expansion::pipelines::Pipeline,
    parser::lexers::assignments::{KeyBuf, Operator, Primitive},
    shell::{IonError, Shell},
    types,
};
use err_derive::Error;
use smallvec::SmallVec;

#[derive(Debug, Error, PartialEq, Eq, Hash)]
pub enum BlockError {
    #[error(display = "Case found outside of Match block")]
    LoneCase,
    #[error(display = "statement found outside of Case block in Match")]
    StatementOutsideMatch,

    #[error(display = "End found but no block to close")]
    UnmatchedEnd,
    #[error(display = "found ElseIf without If block")]
    LoneElseIf,
    #[error(display = "found Else without If block")]
    LoneElse,
    #[error(display = "Else block already exists")]
    MultipleElse,
    #[error(display = "ElseIf found after Else")]
    ElseWrongOrder,

    #[error(display = "found Break without loop body")]
    UnmatchedBreak,
    #[error(display = "found Continue without loop body")]
    UnmatchedContinue,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ElseIf<'a> {
    pub expression: Block<'a>,
    pub success:    Block<'a>,
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
pub struct Case<'a> {
    pub value:       Option<String>,
    pub binding:     Option<String>,
    pub conditional: Option<String>,
    pub statements:  Block<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum LocalAction {
    List,
    Assign(String, Operator, String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ExportAction {
    List,
    LocalExport(String),
    Assign(String, Operator, String),
}

#[derive(Debug, PartialEq, Clone, Copy, Hash)]
pub enum IfMode {
    Success,
    ElseIf,
    Else,
}

// TODO: Enable statements and expressions to contain &str values.
#[derive(Debug, PartialEq, Clone)]
pub enum Statement<'a> {
    Let(LocalAction),
    Case(Case<'a>),
    Export(ExportAction),
    If {
        expression: Block<'a>,
        success:    Block<'a>,
        else_if:    Vec<ElseIf<'a>>,
        failure:    Block<'a>,
        mode:       IfMode,
    },
    ElseIf(ElseIf<'a>),
    Function {
        name:        types::Str,
        description: Option<types::Str>,
        args:        Vec<KeyBuf>,
        statements:  Block<'a>,
    },
    For {
        variables:  SmallVec<[types::Str; 4]>,
        values:     Vec<types::Str>,
        statements: Block<'a>,
    },
    While {
        expression: Block<'a>,
        statements: Block<'a>,
    },
    Match {
        expression: types::Str,
        cases:      Vec<Case<'a>>,
    },
    Else,
    End,
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
    pub fn short(&self) -> &'static str {
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

pub type Block<'a> = Vec<Statement<'a>>;

pub fn insert_statement<'a>(
    block: &mut Block<'a>,
    statement: Statement<'a>,
) -> Result<Option<Statement<'a>>, BlockError> {
    match statement {
        // Push new block to stack
        Statement::For { .. }
        | Statement::While { .. }
        | Statement::Match { .. }
        | Statement::If { .. }
        | Statement::Function { .. } => {
            block.push(statement);
            Ok(None)
        }
        // Case is special as it should pop back previous Case
        Statement::Case(_) => {
            match block.last() {
                Some(Statement::Case(_)) => {
                    let case = block.pop().unwrap();
                    let _ = insert_into_block(block, case);
                }
                Some(Statement::Match { .. }) => (),
                _ => return Err(BlockError::LoneCase),
            }

            block.push(statement);
            Ok(None)
        }
        Statement::End => {
            match block.len() {
                0 => Err(BlockError::UnmatchedEnd),
                // Ready to return the complete block
                1 => Ok(block.pop()),
                // Merge back the top block into the previous one
                _ => {
                    let last_statement = block.pop().unwrap();
                    if let Statement::Case(_) = last_statement {
                        insert_into_block(block, last_statement)?;
                        // Merge last Case back and pop off Match too
                        let match_stm = block.pop().unwrap();
                        if !block.is_empty() {
                            insert_into_block(block, match_stm)?;

                            Ok(None)
                        } else {
                            Ok(Some(match_stm))
                        }
                    } else {
                        insert_into_block(block, last_statement)?;
                        Ok(None)
                    }
                }
            }
        }
        Statement::And(_) | Statement::Or(_) if !block.is_empty() => {
            let pushed = match block.last_mut().unwrap() {
                Statement::If {
                    ref mut expression,
                    ref mode,
                    ref success,
                    ref mut else_if,
                    ..
                } => match mode {
                    IfMode::Success if success.is_empty() => {
                        // Insert into If expression if there's no previous statement.
                        expression.push(statement.clone());
                        true
                    }
                    IfMode::ElseIf => {
                        // Try to insert into last ElseIf expression if there's no previous
                        // statement.
                        let eif = else_if.last_mut().expect("Missmatch in 'If' mode!");
                        if eif.success.is_empty() {
                            eif.expression.push(statement.clone());
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                },
                Statement::While { ref mut expression, ref statements } => {
                    if statements.is_empty() {
                        expression.push(statement.clone());
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !pushed {
                insert_into_block(block, statement)?;
            }

            Ok(None)
        }
        Statement::Time(inner) => {
            if inner.is_block() {
                block.push(Statement::Time(inner));
                Ok(None)
            } else {
                Ok(Some(Statement::Time(inner)))
            }
        }
        _ => {
            if !block.is_empty() {
                insert_into_block(block, statement)?;
                Ok(None)
            } else {
                // Filter out toplevel statements that should produce an error
                // otherwise return the statement for immediat execution
                match statement {
                    Statement::ElseIf(_) => Err(BlockError::LoneElseIf),
                    Statement::Else => Err(BlockError::LoneElse),
                    Statement::Break => Err(BlockError::UnmatchedBreak),
                    Statement::Continue => Err(BlockError::UnmatchedContinue),
                    // Toplevel statement, return to execute immediately
                    _ => Ok(Some(statement)),
                }
            }
        }
    }
}

fn insert_into_block<'a>(
    block: &mut Block<'a>,
    statement: Statement<'a>,
) -> Result<(), BlockError> {
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
                return Err(BlockError::StatementOutsideMatch);
            }
        },
        Statement::Case(ref mut case) => case.statements.push(statement),
        Statement::If {
            ref mut success, ref mut else_if, ref mut failure, ref mut mode, ..
        } => match statement {
            Statement::ElseIf(eif) => {
                if *mode == IfMode::Else {
                    return Err(BlockError::ElseWrongOrder);
                } else {
                    *mode = IfMode::ElseIf;
                    else_if.push(eif);
                }
            }
            Statement::Else => {
                if *mode == IfMode::Else {
                    return Err(BlockError::MultipleElse);
                } else {
                    *mode = IfMode::Else;
                }
            }
            _ => match mode {
                IfMode::Success => success.push(statement),
                IfMode::ElseIf => else_if.last_mut().unwrap().success.push(statement),
                IfMode::Else => failure.push(statement),
            },
        },
        _ => unreachable!("Not block-like statement pushed to stack!"),
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Function<'a> {
    description: Option<types::Str>,
    name:        types::Str,
    args:        Vec<KeyBuf>,
    statements:  Block<'a>,
}

#[derive(Debug, PartialEq, Clone, Error)]
pub enum FunctionError {
    #[error(display = "invalid number of arguments supplied")]
    InvalidArgumentCount,
    #[error(display = "argument has invalid type: expected {}, found value '{}'", _0, _1)]
    InvalidArgumentType(Primitive, String),
}

impl<'a> Function<'a> {
    pub fn is_empty(&self) -> bool { self.statements.is_empty() }

    pub fn execute<S: AsRef<str>>(
        &self,
        shell: &mut Shell<'a>,
        args: &[S],
    ) -> Result<(), IonError> {
        if args.len() - 1 != self.args.len() {
            Err(FunctionError::InvalidArgumentCount)?;
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
            shell.variables.set(&type_.name, value);
        }

        let res = shell.execute_statements(&self.statements);

        shell.variables.pop_scope();
        shell.variables.append_scopes(temporary);
        res.map(|_| ())
    }

    pub fn get_description(&self) -> Option<&types::Str> { self.description.as_ref() }

    pub fn new(
        description: Option<types::Str>,
        name: types::Str,
        args: Vec<KeyBuf>,
        statements: Vec<Statement<'a>>,
    ) -> Self {
        Function { description, name, args, statements }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_match() -> Statement<'static> {
        Statement::Match { expression: types::Str::from(""), cases: Vec::new() }
    }
    fn new_if() -> Statement<'static> {
        Statement::If {
            expression: vec![Statement::Default],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       IfMode::Success,
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
        let mut flow_control = Block::default();

        let res = insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.len(), 1);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        // Pops back top case, len stays 2
        let res = insert_statement(&mut flow_control, new_case());
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, new_if());
        assert_eq!(flow_control.len(), 3);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.len(), 2);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::End);
        assert_eq!(flow_control.len(), 0);
        if let Ok(Some(Statement::Match { ref cases, .. })) = res {
            assert_eq!(cases.len(), 2);
            assert_eq!(cases.last().unwrap().statements.len(), 1);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn statement_outside_case() {
        let mut flow_control = Block::default();

        let res = insert_statement(&mut flow_control, new_match());
        assert_eq!(flow_control.len(), 1);
        assert_eq!(res, Ok(None));

        let res = insert_statement(&mut flow_control, Statement::Default);
        if res.is_err() {
            flow_control.clear();
            assert_eq!(flow_control.len(), 0);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn return_toplevel() {
        let mut flow_control = Block::default();
        let oks = vec![
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
