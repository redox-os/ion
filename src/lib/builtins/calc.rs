use calc::{eval, eval_polish, CalcError, Value};
use std::io::{self, Write};

fn calc_or_polish_calc(args: String) -> Result<Value, CalcError> {
    match eval(&args) {
        Ok(t) => Ok(t),
        Err(_) => eval_polish(&args),
    }
}

pub(crate) fn calc(args: &[String]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if !args.is_empty() {
        let result = calc_or_polish_calc(args.join(" "));
        let _ = match result {
            Ok(v) => writeln!(stdout, "{}", v),
            Err(e) => writeln!(stdout, "{}", e),
        };
    } else {
        let prompt = b"ion-calc: ";
        loop {
            let _ = stdout.write(prompt);
            let _ = stdout.flush();
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);
            if input.is_empty() {
                break;
            } else {
                match input.trim() {
                    "" => (),
                    "exit" => break,
                    s => {
                        let result = calc_or_polish_calc(s.to_string());
                        let _ = match result {
                            Ok(v) => writeln!(stdout, "{}", v),
                            Err(e) => writeln!(stdout, "{}", e),
                        };
                    }
                }
            }
        }
    }
    Ok(())
}
