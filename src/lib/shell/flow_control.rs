use lexers::assignments::{KeyBuf, Operator, Primitive};
use parser::{assignments::*, pipelines::Pipeline};
use shell::{flow::FlowLogic, Shell};
use small;
use smallvec::SmallVec;
use std::fmt::{self, Display, Formatter};
use types;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ElseIf {
    pub expression: Vec<Statement>,
    pub success:    Vec<Statement>,
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
        expression: Vec<Statement>,
        success:    Vec<Statement>,
        else_if:    Vec<ElseIf>,
        failure:    Vec<Statement>,
        mode:       u8, // {0 = success, 1 = else_if, 2 = failure}
    },
    ElseIf(ElseIf),
    Function {
        name:        types::Str,
        description: Option<small::String>,
        args:        Vec<KeyBuf>,
        statements:  Vec<Statement>,
    },
    For {
        variable:   types::Str,
        values:     Vec<small::String>,
        statements: Vec<Statement>,
    },
    While {
        expression: Vec<Statement>,
        statements: Vec<Statement>,
    },
    Match {
        expression: small::String,
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

#[derive(Clone, Debug)]
pub(crate) struct FlowControl {
    pub block: Vec<Statement>,
}

impl FlowControl {
    /// On error reset FlowControl fields.
    pub(crate) fn reset(&mut self) { self.block.clear() }

    /// Check if there isn't an unfinished block.
    pub(crate) fn unclosed_block(&self) -> bool { self.block.len() > 0 }
}

impl Default for FlowControl {
    fn default() -> FlowControl {
        FlowControl {
            block: Vec::with_capacity(5),
        }
    }
}

pub(crate) fn insert_statement(
    flow_control: &mut FlowControl,
    statement: Statement,
) -> Result<Option<Statement>, &'static str> {
    match statement {
        // Push new block to stack
        Statement::For { .. }
        | Statement::While { .. }
        | Statement::Match { .. }
        | Statement::If { .. }
        | Statement::Function { .. } => flow_control.block.push(statement),
        // Case is special as it should pop back previous Case
        Statement::Case(_) => {
            let mut top_is_case = false;
            match flow_control.block.last() {
                Some(Statement::Case(_)) => top_is_case = true,
                Some(Statement::Match { .. }) => (),
                _ => return Err("ion: error: Case { .. } found outside of Match { .. } block"),
            }

            if top_is_case {
                let case = flow_control.block.pop().unwrap();
                let _ = insert_into_block(&mut flow_control.block, case);
            }
            flow_control.block.push(statement);
        }
        Statement::End => {
            match flow_control.block.len() {
                0 => return Err("ion: error: keyword End found but no block to close"),
                // Ready to return the complete block
                1 => return Ok(flow_control.block.pop()),
                // Merge back the top block into the previous one
                _ => {
                    let block = flow_control.block.pop().unwrap();
                    match block {
                        Statement::Case(_) => {
                            // Merge last Case back and pop off Match too
                            insert_into_block(&mut flow_control.block, block)?;
                            let match_stm = flow_control.block.pop().unwrap();
                            if flow_control.block.len() > 0 {
                                insert_into_block(&mut flow_control.block, match_stm)?;
                            } else {
                                return Ok(Some(match_stm));
                            }
                        }
                        _ => insert_into_block(&mut flow_control.block, block)?,
                    }
                }
            }
        }
        Statement::And(_) | Statement::Or(_) if flow_control.block.len() > 0 => {
            let mut pushed = true;
            if let Some(top) = flow_control.block.last_mut() {
                match top {
                    Statement::If {
                        ref mut expression,
                        ref mode,
                        ref success,
                        ref mut else_if,
                        ..
                    } => match *mode {
                        0 if success.len() == 0 => {
                            // Insert into If expression if there's no previous statement.
                            expression.push(statement.clone());
                        }
                        1 => {
                            // Try to insert into last ElseIf expression if there's no previous
                            // statement.
                            if let Some(mut eif) = else_if.last_mut() {
                                if eif.success.len() == 0 {
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
                    Statement::While {
                        ref mut expression,
                        ref statements,
                    } => if statements.len() == 0 {
                        expression.push(statement.clone());
                    } else {
                        pushed = false;
                    },
                    _ => pushed = false,
                }
            } else {
                unreachable!()
            }
            if !pushed {
                insert_into_block(&mut flow_control.block, statement)?;
            }
        }
        _ => if flow_control.block.len() > 0 {
            insert_into_block(&mut flow_control.block, statement)?;
        } else {
            // Filter out toplevel statements that should produce an error
            // otherwise return the statement for immediat execution
            match statement {
                Statement::ElseIf(_) => {
                    return Err("ion: error: found ElseIf { .. } without If { .. } block")
                }
                Statement::Else => return Err("ion: error: found Else without If { .. } block"),
                Statement::Break => return Err("ion: error: found Break without loop body"),
                Statement::Continue => return Err("ion: error: found Continue without loop body"),
                // Toplevel statement, return to execute immediately
                _ => return Ok(Some(statement)),
            }
        },
    }
    Ok(None)
}

fn insert_into_block(block: &mut Vec<Statement>, statement: Statement) -> Result<(), &'static str> {
    if let Some(top_block) = block.last_mut() {
        match top_block {
            Statement::Function {
                ref mut statements, ..
            } => statements.push(statement),
            Statement::For {
                ref mut statements, ..
            } => statements.push(statement),
            Statement::While {
                ref mut statements, ..
            } => statements.push(statement),
            Statement::Match { ref mut cases, .. } => match statement {
                Statement::Case(case) => cases.push(case),
                _ => {
                    return Err(
                        "ion: error: statement found outside of Case { .. } block in Match { .. }",
                    );
                }
            },
            Statement::Case(ref mut case) => case.statements.push(statement),
            Statement::If {
                ref mut success,
                ref mut else_if,
                ref mut failure,
                ref mut mode,
                ..
            } => match statement {
                Statement::ElseIf(eif) => if *mode > 1 {
                    return Err("ion: error: ElseIf { .. } found after Else");
                } else {
                    *mode = 1;
                    else_if.push(eif);
                },
                Statement::Else => if *mode == 2 {
                    return Err("ion: error: Else block already exists");
                } else {
                    *mode = 2;
                },
                _ => match *mode {
                    0 => success.push(statement),
                    1 => else_if.last_mut().unwrap().success.push(statement),
                    2 => failure.push(statement),
                    _ => unreachable!(),
                },
            },
            _ => unreachable!("Not block-like statement pushed to stack!"),
        }
    } else {
        unreachable!("Should not insert statement if stack is empty!")
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    description: Option<small::String>,
    name:        types::Str,
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
            InvalidArgumentType(ref t, ref value) => write!(fmt, "{} is not of type {}", value, t),
        }
    }
}

impl Function {
    pub fn is_empty(&self) -> bool { self.statements.is_empty() }

    pub(crate) fn execute<S: AsRef<str>>(
        self,
        shell: &mut Shell,
        args: &[S],
    ) -> Result<(), FunctionError> {
        if args.len() - 1 != self.args.len() {
            return Err(FunctionError::InvalidArgumentCount);
        }

        let name = self.name.clone();

        let mut values: SmallVec<[_; 8]> = SmallVec::new();

        for (type_, value) in self.args.iter().zip(args.iter().skip(1)) {
            let value = match value_check(shell, value.as_ref(), &type_.kind) {
                Ok(value) => value,
                Err(_) => {
                    return Err(FunctionError::InvalidArgumentType(
                        type_.kind.clone(),
                        value.as_ref().into(),
                    ))
                }
            };

            values.push((type_.clone(), value));
        }

        let index = shell
            .variables
            .index_scope_for_var(&name)
            .expect("execute called with invalid function");

        // Pop off all scopes since function temporarily
        let temporary: Vec<_> = shell.variables.pop_scopes(index).collect();

        shell.variables.new_scope(true);

        for (type_, value) in values {
            shell.variables.shadow(&type_.name, value);
        }

        shell.execute_statements(self.statements);

        shell.variables.pop_scope();
        shell.variables.append_scopes(temporary);
        Ok(())
    }

    pub(crate) fn get_description<'a>(&'a self) -> Option<&'a small::String> {
        self.description.as_ref()
    }

    pub(crate) fn new(
        description: Option<small::String>,
        name: types::Str,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn new_match() -> Statement {
        Statement::Match {
            expression: small::String::from(""),
            cases:      Vec::new(),
        }
    }
    fn new_if() -> Statement {
        Statement::If {
            expression: vec![Statement::Default],
            success:    Vec::new(),
            else_if:    Vec::new(),
            failure:    Vec::new(),
            mode:       0,
        }
    }
    fn new_case() -> Statement {
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
        if let Err(_) = res {
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

        let errs = vec![
            Statement::Else,
            Statement::End,
            Statement::Break,
            Statement::Continue,
        ];
        for err in errs {
            let res = insert_statement(&mut flow_control, err);
            if let Ok(_) = res {
                assert!(false);
            }
        }
    }
}
