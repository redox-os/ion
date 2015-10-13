use std::process::{Command,Output};

pub struct InstructionOut {
    pub stdout: String,
    pub stderr: String,
}

pub fn run(input_command: Vec<&str>) -> Option<InstructionOut> {
    let args = input_command.as_slice();
    let length = args.len();
    let output: Option<Output>;
    if length ==0 {
        output = Command::new("").output().ok();
    } else if length ==  1 {
        output = Command::new(&args[0]).output().ok();
    } else {
        output = Command::new(&args[0]).args(&args[1..]).output().ok();
    };
    if output.is_some() {
        let output = output.unwrap();
        Some(InstructionOut {
            stdout: String::from_utf8(output.stdout).ok().expect("No stdout"),
            stderr: String::from_utf8(output.stderr).ok().expect("No stderr"),
        })
    } else {
        None
    }
}
