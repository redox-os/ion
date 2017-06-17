use types::Identifier;
use parser::peg::Pipeline;
use parser::assignments::Binding;

#[derive(Debug, PartialEq, Clone)]
pub struct ElseIf {
    pub expression: Pipeline,
    pub success:    Vec<Statement>
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let {
        expression: Binding,
    },
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
        args: Vec<String>,
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
    pub current_if_mode:      u8 // { 0 = SUCCESS; 1 = FAILURE }
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
    pub args: Vec<String>,
    pub statements: Vec<Statement>
}

pub fn collect_loops<I>(iterator: &mut I, statements: &mut Vec<Statement>, level: &mut usize)
    where I: Iterator<Item = Statement>
{
    #[allow(while_let_on_iterator)]
    while let Some(statement) = iterator.next() {
        match statement {
            Statement::While{..} | Statement::For{..} | Statement::If{..} |
                Statement::Function{..} => *level += 1,
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
                Statement::Function{..} => *level += 1,
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
