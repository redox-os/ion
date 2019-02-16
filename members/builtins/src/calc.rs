use calculate::{eval, eval_polish, CalcError, Value};
use small;
use std::io::{self, Write};

const REPL_GUIDE: &str = r#"ion-calc
Type in expressions to have them evaluated.
Type "help" for help."#;

pub const MAN_CALC: &str = r#"NAME
    calc - Floating point calculator

SYNOPSIS
    calc [EXPRESSION]

DESCRIPTION
    Evaluates arithmetic expressions

SPECIAL EXPRESSIONS
    help (only in interactive mode)
        prints this help text

    --help (only in non-interactive mode)
        prints this help text

    exit (only in interactive mode)
        exits the program

NOTATIONS
    infix notation
        e.g. 3 * 4 + 5

    polish notation
        e.g. + * 3 4 5

EXAMPLES
    Add two plus two in infix notation
        calc 2+2

    Add two plus two in polish notation
        calc + 2 2

AUTHOR
    Written by Hunter Goldstein.
"#;

fn calc_or_polish_calc(args: &str) -> Result<Value, CalcError> {
    match eval(&args) {
        Ok(t) => Ok(t),
        Err(_) => eval_polish(&args),
    }
}

pub fn calc(args: &[small::String]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    match args.first() {
        Some(ref s) if (*s == "--help") => {
            // "--help" only makes sense if it is the first option. Only look for it
            // in the first position.
            println!("{}", MAN_CALC);
            Ok(())
        }
        Some(_) => {
            let result = calc_or_polish_calc(&args.join(" "));
            let _ = match result {
                Ok(v) => writeln!(stdout, "{}", v),
                Err(e) => writeln!(stdout, "{}", e),
            };
            Ok(())
        }
        None => {
            let prompt = b"ion-calc: ";
            println!("{}", REPL_GUIDE);
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
                        "help" => println!("{}", MAN_CALC),
                        s => {
                            let result = calc_or_polish_calc(s);
                            let _ = match result {
                                Ok(v) => writeln!(stdout, "{}", v),
                                Err(e) => writeln!(stdout, "{}", e),
                            };
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
