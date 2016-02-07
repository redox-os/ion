use super::to_num::ToNum;

pub fn is_flow_control_command(command: &str) -> bool {
    command == "end" || command == "if" || command == "else"
}

pub struct Mode {
    pub value: bool,
}

pub struct FlowControl {
    pub modes: Vec<Mode>,
}

impl FlowControl {
    pub fn new() -> FlowControl {
        FlowControl { modes: vec![] }
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
            if let Some(cmp) = args.nth(0) {
                if let Some(right) = args.nth(0) {
                    if cmp.as_ref() == "==" {
                        value = left.as_ref() == right.as_ref();
                    } else if cmp.as_ref() == "!=" {
                        value = left.as_ref() != right.as_ref();
                    } else if cmp.as_ref() == ">" {
                        value = left.as_ref().to_num_signed() > right.as_ref().to_num_signed();
                    } else if cmp.as_ref() == ">=" {
                        value = left.as_ref().to_num_signed() >= right.as_ref().to_num_signed();
                    } else if cmp.as_ref() == "<" {
                        value = left.as_ref().to_num_signed() < right.as_ref().to_num_signed();
                    } else if cmp.as_ref() == "<=" {
                        value = left.as_ref().to_num_signed() <= right.as_ref().to_num_signed();
                    } else {
                        println!("Unknown comparison: {}", cmp.as_ref());
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
}
