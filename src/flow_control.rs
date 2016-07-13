use super::peg::Pipeline;

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    If {
        left: String,
        comparitor: Comparitor,
        right: String
    },
    Function{
        name: String,
        args: Vec<String>
    },
    For{
        variable: String,
        values: Vec<String>
    },
    Else,
    End,
    Pipelines(Vec<Pipeline>),
    Default
}

impl Statement {

    pub fn is_flow_control(&self) -> bool {
        match *self {
            Statement::If{..}  |
            Statement::Else    |
            Statement::For{..} |
            Statement::Function{..} => true,

            Statement::End           |
            Statement::Pipelines(..) |
            Statement::Default      => false

        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Comparitor {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual
}

pub struct CodeBlock {
    pub pipelines: Vec<Pipeline>,
}

pub struct Mode {
    pub value: bool,
}

pub struct FlowControl {
    pub modes: Vec<Mode>,
    pub collecting_block: bool,
    pub current_block: CodeBlock,
    pub current_statement: Statement, /* pub prompt: &'static str,  // Custom prompt while collecting code block */
}

impl Default for FlowControl {
    fn default() -> FlowControl {
        FlowControl {
            modes: vec![],
            collecting_block: false,
            current_block: CodeBlock { pipelines: vec![] },
            current_statement: Statement::Default,
        }
    }
}
