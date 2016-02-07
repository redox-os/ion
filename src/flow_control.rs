use super::to_num::ToNum;
use super::peg::Job;

pub fn is_flow_control_command(command: &str) -> bool {
    command == "end" || command == "if" || command == "else"
}

pub enum Statement {
    For(String, Vec<String>),
    Default,
}

pub struct CodeBlock {
    pub jobs: Vec<Job>,
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

impl FlowControl {
    pub fn new() -> FlowControl {
        FlowControl {
            modes: vec![],
            collecting_block: false,
            current_block: CodeBlock { jobs: vec![] },
            current_statement: Statement::Default,
        }
    }

    pub fn skipping(&self) -> bool {
        self.modes.iter().any(|mode| !mode.value)
    }

    pub fn if_<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter(); // TODO why does the compiler want this to be mutable?
        let mut value = false;
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
                    }
                } else {
                    println!("No right hand side");
                }
            } else {
                println!("No comparison operator");
            }
        } else {
            println!("No left hand side");
        }
        self.modes.insert(0, Mode { value: value });
    }

    pub fn else_<I: IntoIterator>(&mut self, _: I)
        where I::Item: AsRef<str>
    {
        if let Some(mode) = self.modes.get_mut(0) {
            mode.value = !mode.value;
        } else {
            println!("Syntax error: else found with no previous if");
        }
    }

    pub fn end<I: IntoIterator>(&mut self, _: I)
        where I::Item: AsRef<str>
    {
        if !self.modes.is_empty() {
            self.modes.remove(0);
        } else {
            println!("Syntax error: fi found with no previous if");
        }
    }

    pub fn for_<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter();
        if let Some(variable) = args.nth(1).map(|var| var.as_ref().to_string()) {
            if let Some(in_) = args.nth(0) {
                if in_.as_ref() != "in" {
                    println!("For loops must have 'in' as the second argument");
                    return;
                }
            } else {
                println!("For loops must have 'in' as the second argument");
                return;
            }
            let values: Vec<String> = args.map(|value| value.as_ref().to_string()).collect();
            self.current_statement = Statement::For(variable, values);
            self.collecting_block = true;
        } else {
            println!("For loops must have a variable name as the first argument");
            return;
        }
    }
}
