use super::peg::Pipeline;
use directory_stack::DirectoryStack;
use variables::Variables;
use shell_expand::words::{WordIterator, WordToken};
use shell_expand::{braces, variables};

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
        values: String,
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

pub fn parse_for(expression: &str, dir_stack: &DirectoryStack, variables: &Variables) -> String {
    let mut output = String::new();
    let mut word_iterator = WordIterator::new(expression);

    let expand_variable = |variable: &str, _: bool| {
        variables.get_var(variable)
    };
    let expand_command = |command:  &str, quoted: bool| {
        variables.command_expansion(command, quoted)
    };

    while let Some(Ok(word)) = word_iterator.next() {
        match word {
            WordToken::Brace(text, contains_variables) => {
                if contains_variables {
                    let mut temp = String::new();
                    variables::expand(&mut temp, text,
                        |variable| expand_variable(variable, false),
                        |command| expand_command(command, false)
                    );
                    braces::expand_braces(&mut output, &temp);
                } else {
                    braces::expand_braces(&mut output, text);
                }
            },
            WordToken::Normal(expr) => output.push_str(expr),
            WordToken::Tilde(tilde) => match variables.tilde_expansion(tilde, dir_stack) {
                Some(expanded) => output.push_str(&expanded),
                None           => output.push_str(tilde),
            },
            WordToken::Variable(text, quoted) => {
                variables::expand(&mut output, text,
                    |variable| expand_variable(variable, quoted),
                    |command| expand_command(command, quoted)
                );
            }
        }
    }

    output
}