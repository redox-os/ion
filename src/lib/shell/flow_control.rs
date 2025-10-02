use crate::{
    assignments::*,
    expansion::pipelines::Pipeline,
    parser::lexers::assignments::{KeyBuf, Operator, Primitive},
    shell::{IonError, Job, Shell},
    types,
};
use smallvec::SmallVec;
use std::fmt;
use thiserror::Error;

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
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Case {
    /// The value to match with
    pub value:       Option<String>,
    /// Set a variable with the exact result
    pub binding:     Option<String>,
    /// An additional statement to test before matching the case statement
    pub conditional: Option<String>,
    /// The block to execute on matching input
    pub statements:  Block,
}

/// An elseif case
#[derive(Debug, PartialEq, Clone)]
pub struct ElseIf {
    /// The block to test
    pub expression: Block,
    /// The block to execute on success
    pub success:    Block,
}

/// The action to perform on assignment
#[derive(Debug, PartialEq, Clone)]
pub enum LocalAction {
    /// List all the variables
    List,
    /// Assign a value to a name
    Assign(String, Operator, String),
}

/// The action to perform on export
#[derive(Debug, PartialEq, Clone)]
pub enum ExportAction {
    /// List the environment variables
    List,
    /// Export the value
    LocalExport(String),
    /// Export and update
    Assign(String, Operator, String),
}

/// The mode for the next if block
#[derive(Debug, PartialEq, Clone, Copy, Hash)]
pub enum IfMode {
    /// Standard if
    Success,
    /// Else if
    ElseIf,
    /// Else
    Else,
}

/// A single statement
///
/// Contains all the possible actions for the shell
// TODO: Enable statements and expressions to contain &str values.
#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    /// Assignment
    Let(LocalAction),
    /// A case
    Case(Case),
    /// Export a variable
    Export(ExportAction),
    /// An if block
    If {
        /// The block to test
        expression: Block,
        /// The block to execute on success
        success:    Block,
        /// The list of associated else if blocks
        else_if:    Vec<ElseIf>,
        /// The block to execute on failure
        failure:    Block,
        /// The mode
        mode:       IfMode,
    },
    /// else if
    ElseIf(ElseIf),
    /// Create a function
    Function {
        /// the name of the function
        name:        types::Str,
        /// the description of the function
        description: Option<types::Str>,
        /// The required arguments of the function, with their types
        args:        Vec<KeyBuf>,
        /// The statements in the function
        statements:  Block,
    },
    /// for loop
    For {
        /// The bounds
        variables:  SmallVec<[types::Str; 4]>,
        /// The value to iterator for
        values:     Vec<types::Str>,
        /// The block to execute repetitively
        statements: Block,
    },
    /// while
    While {
        /// The block to test
        expression: Block,
        /// The block to execute repetitively
        statements: Block,
    },
    /// Match
    Match {
        /// The value to check
        expression: types::Str,
        /// A list of case to check for
        cases:      Vec<Case>,
    },
    /// Else statement
    Else,
    /// End of a block
    End,
    /// Exit loop
    Break,
    /// Next loop
    Continue,
    /// Exit from the current function/script
    Return(Option<types::Str>),
    /// Execute a pipeline
    Pipeline(Pipeline<Job>),
    /// Time the statement
    Time(Box<Statement>),
    /// Execute the statement if the previous command succeeded
    And(Box<Statement>),
    /// Execute the statement if the previous command failed
    Or(Box<Statement>),
    /// Succeed on failure of the inner statement
    Not(Box<Statement>),
    /// Assign an environment variable, then execute
    LocalAssignAct {
        /// The variable name
        key:   String,
        /// The variable value
        value: String,
        /// The statement to execute with the assigned variable
        stmt:  Box<Statement>,
    },
    /// An empty statement
    Default,
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
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
                Statement::Return(_) => "Return",
                Statement::LocalAssignAct { key: _, value: _, stmt: _ } => "LocalAssignAct { .. }",
                Statement::Default => "Default",
            }
        )
    }
}

impl Statement {
    /// Check if the statement is a block-based statement
    #[must_use]
    pub const fn is_block(&self) -> bool {
        matches!(
            *self,
            Statement::Case(_)
                | Statement::If { .. }
                | Statement::ElseIf(_)
                | Statement::Function { .. }
                | Statement::For { .. }
                | Statement::While { .. }
                | Statement::Match { .. }
                | Statement::Else
        )
    }
}

/// A collection of statement in a block (delimited by braces in most languages)
pub type Block = Vec<Statement>;

/// A user-defined function
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Function {
    description: Option<types::Str>,
    name:        types::Str,
    args:        Vec<KeyBuf>,
    statements:  Block,
}

/// Error during function execution
#[derive(Debug, PartialEq, Clone, Error)]
pub enum FunctionError {
    /// The wrong number of arguments were supplied
    #[error("invalid number of arguments supplied")]
    InvalidArgumentCount,
    /// The argument had an invalid type
    #[error("argument has invalid type: expected {0}, found value '{1}'")]
    InvalidArgumentType(Primitive, String),
}

impl Function {
    /// execute the function in the shell
    pub fn execute<'a, S: AsRef<str>>(
        &self,
        shell: &mut Shell<'a>,
        args: &[S],
    ) -> Result<(), IonError> {
        if args.len() - 1 != self.args.len() {
            return Err(FunctionError::InvalidArgumentCount.into());
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

    /// Get the function's description
    #[must_use]
    pub const fn description(&self) -> Option<&types::Str> { self.description.as_ref() }

    /// Create a new function
    #[must_use]
    pub const fn new(
        description: Option<types::Str>,
        name: types::Str,
        args: Vec<KeyBuf>,
        statements: Vec<Statement>,
    ) -> Self {
        Self { description, name, args, statements }
    }
}
