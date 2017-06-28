use types::Identifier;
use parser::peg::Pipeline;
use parser::assignments::Binding;

#[derive(Debug, PartialEq, Clone)]
pub struct ElseIf {
    pub expression: Pipeline,
    pub success:    Vec<Statement>
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Type { Float, Int, Bool }

#[derive(Debug, PartialEq, Clone)]
pub enum FunctionArgument { Typed(String, Type), Untyped(String) }


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
    pub statements: Vec<Statement>
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let {
        expression: Binding,
    },
    Case(Case),
    Export(Binding),
    If {
        expression: Pipeline,
        success: Vec<Statement>,
        else_if: Vec<ElseIf>,
        failure: Vec<Statement>
    },
    ElseIf(ElseIf),
    Function {
        name: Identifier,
        description: String,
        args: Vec<FunctionArgument>,
        statements: Vec<Statement>
    },
    For {
        variable: Identifier,
        values: Vec<String>,
        statements: Vec<Statement>
    },
    While {
        expression: Pipeline,
        statements: Vec<Statement>
    },
    Match {
        expression: String,
        cases : Vec<Case>
    },
    Else,
    End,
    Error(i32),
    Break,
    Continue,
    Pipeline(Pipeline),
    Default
}

pub struct FlowControl {
    pub level:             usize,
    pub current_statement: Statement,
    pub current_if_mode:   u8 // { 0 = SUCCESS; 1 = FAILURE }
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

#[derive(Clone)]
pub struct Function {
    pub description: String,
    pub name: Identifier,
    pub args: Vec<FunctionArgument>,
    pub statements: Vec<Statement>
}

macro_rules! add_to_case {
    ($cases:expr, $statement:expr) => {
        match $cases.last_mut() {
            // XXX: When does this actually happen? What syntax error is this???
            None => return Err("ion: syntax error: encountered ... outside of `case ... end` block".into()),
            Some(ref mut case) => case.statements.push($statement),
        }
    }
}

pub fn collect_cases<I>(iterator: &mut I, cases: &mut Vec<Case>, level: &mut usize) -> Result<(), String>
    where I : Iterator<Item=Statement>
{
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::Case(case) => {
                *level += 1;
                if *level == 2 {
                    // When the control flow level equals two, this means we are inside the
                    // body of the match statement and should treat this as the new case of _this_
                    // match. Otherwise we will just add it to the current case.
                    cases.push(case);
                } else {
                    add_to_case!(cases, Statement::Case(case));
                }
            },
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
                if *level < 2 {
                    // If the level is less than two, then this statement has appeared outside
                    // of a block delimited by a case...end pair

                    // XXX: This syntax error is very unhelpful as it does not tell us _what_ we
                    // got. However if we include the full debug information its very noisy. We
                    // should write a function that returns a short form version of what we found.
                    return Err("ion: syntax error: expected end or case, got ...".into());
                } else {
                    // Otherwise it means we've hit a case statement for some other match construct
                    *level += 1;
                    add_to_case!(cases, statement);
                }
            },
            Statement::Default |
            Statement::Else |
            Statement::ElseIf { .. } |
            Statement::Error(_) |
            Statement::Export(_) |
            Statement::Continue |
            Statement::Let { .. } |
            Statement::Pipeline(_) |
            Statement::Break => {
                // This is the default case with all of the other statements explicitly listed
                add_to_case!(cases, statement);
            },
        }
    }
    return Ok(());
}

pub fn collect_loops <I: Iterator<Item = Statement>> (
    iterator: &mut I,
    statements: &mut Vec<Statement>,
    level: &mut usize
) {
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While{..} | Statement::For{..} | Statement::If{..} |
                Statement::Function{..} | Statement::Match{..} | Statement::Case{..} => *level += 1,
            Statement::End if *level == 1 => { *level = 0; break },
            Statement::End => *level -= 1,
            _ => (),
        }
        statements.push(statement);
    }
}

pub fn collect_if<I>(iterator: &mut I, success: &mut Vec<Statement>, else_if: &mut Vec<ElseIf>,
    failure: &mut Vec<Statement>, level: &mut usize, mut current_block: u8)
        -> Result<u8, &'static str>
    where I: Iterator<Item = Statement>
{
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While{..} | Statement::For{..} | Statement::If{..} |
                Statement::Function{..} | Statement::Match{..} | Statement::Case{..} => *level += 1,
            Statement::ElseIf(ref elseif) if *level == 1 => {
                if current_block == 1 {
                    return Err("ion: syntax error: else block already given");
                } else {
                    current_block = 2;
                    else_if.push(elseif.clone());
                    continue
                }
            }
            Statement::Else if *level == 1 => {
                current_block = 1;
                continue
            },
            Statement::Else if *level == 1 && current_block == 1 => {
                return Err("ion: syntax error: else block already given");
            }
            Statement::End if *level == 1 => { *level = 0; break },
            Statement::End => *level -= 1,
            _ => (),
        }

        match current_block {
            0 => success.push(statement),
            1 => failure.push(statement),
            2 => {
                let mut last = else_if.last_mut().unwrap(); // This is a bug if there isn't a value
                last.success.push(statement);
            }
            _ => unreachable!()
        }
    }

    Ok(current_block)
}
