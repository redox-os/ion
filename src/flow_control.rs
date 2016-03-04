use super::to_num::ToNum;
use super::peg::Job;
use super::status::{SUCCESS, FAILURE};
use std::default::Default;

pub fn is_flow_control_command(command: &str) -> bool {
    command == "end" || command == "if" || command == "else"
}

#[derive(Clone)]
pub enum Statement {
    For(String, Vec<String>),
    Function(String),
    If(bool),
    Default,
}

#[derive(Clone)]
pub enum Expression {
    Job(Job),
    Block(CodeBlock)
}

#[derive(Clone)]
pub struct CodeBlock {
    pub expressions: Vec<Expression>,
    pub statement: Statement,
    pub collecting: bool,
}

impl CodeBlock {
    pub fn new(statement: Statement, collecting: bool) -> CodeBlock {
        CodeBlock { expressions: vec![], statement: statement, collecting: collecting }
    }
}

impl Default for CodeBlock {
    fn default() -> CodeBlock {
        CodeBlock::new(Statement::Default, false)
    }
}

pub struct Mode {
    pub value: bool,
}

pub struct FlowControl {
    pub blocks: Vec<CodeBlock>,
}

impl FlowControl {
    pub fn new() -> FlowControl {
        FlowControl {
            blocks: vec! [CodeBlock::new(Statement::Default, false)],
            // current_block: CodeBlock { jobs: vec![] },
            // statements: vec![Statement::Default],
        }
    }

    pub fn skipping(&self) -> bool {
        self.blocks.iter().any(|block| match block.statement {
            Statement::If(value) => !value,
            _ => false
        })
    }

    pub fn if_<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter(); // TODO why does the compiler want this to be mutable?
        let value;
        if let Some(left) = args.nth(1) {
            let left = left.as_ref();
            if let Some(cmp) = args.nth(0) {
                let cmp = cmp.as_ref();
                if let Some(right) = args.nth(0) {
                    let right = right.as_ref();
                    if cmp == "==" {
                        value = left == right;
                    } else if cmp == "!=" {
                        value = left != right;
                    } else if cmp == ">" {
                        value = left.to_num_signed() > right.to_num_signed();
                    } else if cmp == ">=" {
                        value = left.to_num_signed() >= right.to_num_signed();
                    } else if cmp == "<" {
                        value = left.to_num_signed() < right.to_num_signed();
                    } else if cmp == "<=" {
                        value = left.to_num_signed() <= right.to_num_signed();
                    } else {
                        println!("Unknown comparison: {}", cmp);
                        return FAILURE;
                    }
                } else {
                    println!("No right hand side");
                    return FAILURE;
                }
            } else {
                println!("No comparison operator");
                return FAILURE;
            }
        } else {
            println!("No left hand side");
            return FAILURE;
        }
        self.blocks.push(CodeBlock::new(Statement::If(value), false));
        SUCCESS
    }

    pub fn else_<I: IntoIterator>(&mut self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        if let Statement::If(ref mut value) = self.blocks.last_mut().unwrap_or(&mut CodeBlock::default()).statement {
            *value = !*value;
            SUCCESS
        } else {
            println!("Syntax error: else found with no previous if");
            FAILURE
        }
    }

    pub fn end<I: IntoIterator>(&mut self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        if self.blocks.len() > 1{
            self.blocks.pop();
            SUCCESS
        } else {
            println!("Syntax error: end found outside of a block");
            FAILURE
        }
    }

    pub fn for_<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter();
        if let Some(variable) = args.nth(1).map(|var| var.as_ref().to_string()) {
            if let Some(in_) = args.nth(0) {
                if in_.as_ref() != "in" {
                    println!("For loops must have 'in' as the second argument");
                    return FAILURE;
                }
            } else {
                println!("For loops must have 'in' as the second argument");
                return FAILURE;
            }
            let values: Vec<String> = args.map(|value| value.as_ref().to_string()).collect();
            self.blocks.push(CodeBlock::new(Statement::For(variable, values), true));
        } else {
            println!("For loops must have a variable name as the first argument");
            return FAILURE;
        }
        SUCCESS
    }

    pub fn fn_<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter();
        if let Some(name) = args.nth(1) {
            self.blocks.push(CodeBlock::new(Statement::Function(name.as_ref().to_string()), true));
        } else {
            println!("Functions must have the function name as the first argument");
            return FAILURE;
        }
        SUCCESS
    }
}
