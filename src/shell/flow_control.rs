use super::Shell;
use super::flow::FlowLogic;
use fnv::*;
use parser::pipelines::Pipeline;
use types::*;
use types::Identifier;

#[derive(Debug, PartialEq, Clone)]
pub struct ElseIf {
    pub expression: Pipeline,
    pub success: Vec<Statement>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Type {
    Float,
    Int,
    Bool,
}

#[derive(Debug, PartialEq, Clone)]
pub enum FunctionArgument {
    Typed(String, Type),
    Untyped(String),
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
/// Case { value: Some(value), statements: vec![statement0, statement1, ... statementN]}
/// ```
/// The wildcard branch, a branch that matches any value, is represented as such:
/// ```rust,ignore
/// Case { value: None, ... }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct Case {
    pub value: Option<String>,
    pub statements: Vec<Statement>,
}

// TODO: Enable statements and expressions to contain &str values.
#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let { expression: String },
    Case(Case),
    Export(String),
    If {
        expression: Pipeline,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>,
    },
    ElseIf(ElseIf),
    Function {
        name: Identifier,
        description: String,
        args: Vec<FunctionArgument>,
        statements: Vec<Statement>,
    },
    For {
        variable: Identifier,
        values: Vec<String>,
        statements: Vec<Statement>,
    },
    While {
        expression: Pipeline,
        statements: Vec<Statement>,
    },
    Match {
        expression: String,
        cases: Vec<Case>,
    },
    Else,
    End,
    Error(i32),
    Break,
    Continue,
    Pipeline(Pipeline),
    Time(Box<Statement>),
    Default,
}

impl Statement {
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
            Statement::Error(_) => "Error { .. }",
            Statement::Break => "Break",
            Statement::Continue => "Continue",
            Statement::Pipeline(_) => "Pipeline { .. }",
            Statement::Time(_) => "Time { .. }",
            Statement::Default => "Default",

        }
    }
}

pub struct FlowControl {
    pub level: usize,
    pub current_statement: Statement,
    pub current_if_mode: u8, // { 0 = SUCCESS; 1 = FAILURE }
}

impl Default for FlowControl {
    fn default() -> FlowControl {
        FlowControl {
            level: 0,
            current_statement: Statement::Default,
            current_if_mode: 0,
        }
    }
}

#[derive(Clone)]
pub struct Function {
    pub description: String,
    pub name: Identifier,
    pub args: Vec<FunctionArgument>,
    pub statements: Vec<Statement>,
}

pub enum FunctionError {
    InvalidArgumentCount,
    InvalidArgumentType(Type, String),
}

impl Function {
    pub fn execute(self, shell: &mut Shell, args: &[&str]) -> Result<(), FunctionError> {
        if args.len() - 1 != self.args.len() {
            return Err(FunctionError::InvalidArgumentCount);
        }

        let mut variables_backup: FnvHashMap<&str, Option<Value>> =
            FnvHashMap::with_capacity_and_hasher(64, Default::default());

        let mut bad_argument: Option<(&str, Type)> = None;
        for (name_arg, value) in self.args.iter().zip(args.iter().skip(1)) {
            let name: &str = match name_arg {
                &FunctionArgument::Typed(ref name, ref type_) => {
                    match *type_ {
                        Type::Float if value.parse::<f64>().is_ok() => name.as_str(),
                        Type::Int if value.parse::<i64>().is_ok() => name.as_str(),
                        Type::Bool if *value == "true" || *value == "false" => name.as_str(),
                        _ => {
                            bad_argument = Some((value, *type_));
                            break;
                        }
                    }
                }
                &FunctionArgument::Untyped(ref name) => name.as_str(),
            };
            variables_backup.insert(name, shell.variables.get_var(name));
            shell.variables.set_var(name, value);
        }

        match bad_argument {
            Some((actual_value, expected_type)) => {
                for (name, value_option) in &variables_backup {
                    match *value_option {
                        Some(ref value) => shell.variables.set_var(name, value),
                        None => {
                            shell.variables.unset_var(name);
                        }
                    }
                }

                return Err(FunctionError::InvalidArgumentType(expected_type, actual_value.to_owned()));
            }
            None => {
                shell.execute_statements(self.statements);

                for (name, value_option) in &variables_backup {
                    match *value_option {
                        Some(ref value) => shell.variables.set_var(name, value),
                        None => {
                            shell.variables.unset_var(name);
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

pub fn collect_cases<I>(iterator: &mut I, cases: &mut Vec<Case>, level: &mut usize) -> Result<(), String>
    where I: Iterator<Item = Statement>
{

    macro_rules! add_to_case {
        ($statement:expr) => {
            match cases.last_mut() {
                // XXX: When does this actually happen? What syntax error is this???
                None => return Err(["ion: syntax error: encountered ",
                                     $statement.short(),
                                     " outside of `case ...` block"].concat()),
                Some(ref mut case) => case.statements.push($statement),
            }
        }
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
            Statement::While { .. } |
            Statement::For { .. } |
            Statement::If { .. } |
            Statement::Match { .. } |
            Statement::Function { .. } => {
                *level += 1;
                add_to_case!(statement);
            }
            Statement::Default |
            Statement::Else |
            Statement::ElseIf { .. } |
            Statement::Error(_) |
            Statement::Export(_) |
            Statement::Continue |
            Statement::Let { .. } |
            Statement::Pipeline(_) |
            Statement::Time(_) |
            Statement::Break => {
                // This is the default case with all of the other statements explicitly listed
                add_to_case!(statement);
            }
        }
    }
    return Ok(());
}

pub fn collect_loops<I: Iterator<Item = Statement>>(
    iterator: &mut I,
    statements: &mut Vec<Statement>,
    level: &mut usize,
) {
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While { .. } |
            Statement::For { .. } |
            Statement::If { .. } |
            Statement::Function { .. } |
            Statement::Match { .. } => *level += 1,
            Statement::Time(ref box_stmt) => match box_stmt.as_ref() {
                &Statement::While { .. } |
                &Statement::For { .. } |
                &Statement::If { .. } |
                &Statement::Function { .. } |
                &Statement::Match { .. } => *level += 1,
                &Statement::End if *level == 1 => {
                    *level = 0;
                    break;
                }
                &Statement::End => *level -= 1,
                _ => (),
            }
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

pub fn collect_if<I>(
    iterator: &mut I,
    success: &mut Vec<Statement>,
    else_if: &mut Vec<ElseIf>,
    failure: &mut Vec<Statement>,
    level: &mut usize,
    mut current_block: u8,
) -> Result<u8, &'static str>
    where I: Iterator<Item = Statement>
{
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While { .. } |
            Statement::For { .. } |
            Statement::If { .. } |
            Statement::Function { .. } |
            Statement::Match { .. } => *level += 1,
            Statement::ElseIf(ref elseif) if *level == 1 => {
                if current_block == 1 {
                    return Err("ion: syntax error: else block already given");
                } else {
                    current_block = 2;
                    else_if.push(elseif.clone());
                    continue;
                }
            }
            Statement::Else if *level == 1 => {
                current_block = 1;
                continue;
            }
            Statement::Else if *level == 1 && current_block == 1 => {
                return Err("ion: syntax error: else block already given");
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
