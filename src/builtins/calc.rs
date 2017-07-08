extern crate calc;

use std::io::{self, Write};
use calc::{eval, CalcError};

pub fn calc(args: &[&str]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if !args.is_empty() {
        let result = eval(&args.join(""))?;
        writeln!(stdout, "{}", result).map_err(CalcError::IO)?;
    } else {
        let prompt = b"[]> ";
        loop {
            let _ = stdout.write(prompt).map_err(CalcError::IO)?;
            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(CalcError::IO)?;
            if input.is_empty() {
                break;
            } else {
                match input.trim() {
                    "" => (),
                    "exit" => break,
                    s => {
                        writeln!(stdout, "{}", eval(s)?).map_err(CalcError::IO)?;
                    },
                }
            }
        }
    }
    Ok(())
}
