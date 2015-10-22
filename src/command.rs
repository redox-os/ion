use std::process::{Command,Output};

pub struct InstructionOut {
    pub stdout: String,
    pub stderr: String,
}

pub fn run(args: &[&str]) -> Option<InstructionOut> {
    let output: Option<Output>;
    match args.len() {
        0 => output = Command::new("").output().ok(),
        1 => output = Command::new(&args[0]).output().ok(),
        _ => output = Command::new(&args[0]).args(&args[1..]).output().ok(),
    }
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
