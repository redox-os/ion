extern crate calc;

use calc::{CalcError, Value,  eval, eval_polish};
use std::io::{self, Write};

fn calc_or_polish_calc(args: String) -> Result<Value, CalcError>{
    match eval(&args) {
        Ok(t)  => Ok(t),
        Err(_) => eval_polish(&args)
    }
}

pub fn calc(args: &[&str]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if !args.is_empty() {
        let result = calc_or_polish_calc(args.join(" "));
        match result {
            Ok(v)  => writeln!(stdout, "{}", v),
            Err(e) => writeln!(stdout, "{}", e)
        };
    } else {
        let prompt = b"[]> ";
        loop {
            let _ = stdout.write(prompt);
            let mut input = String::new();
            io::stdin().read_line(&mut input);
            if input.is_empty() {
                break;
            } else {
                match input.trim() {
                    "" => (),
                    "exit" => break,
                    s => {
                        let result = calc_or_polish_calc(s.to_string());
                        match result {
                            Ok(v)  => writeln!(stdout, "{}", v),
                            Err(e) => writeln!(stdout, "{}", e)
                        };
                    }
                }
            }
        }
    }
    Ok(())
}
