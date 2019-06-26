use super::Status;
use crate as ion_shell;
use builtins_proc::builtin;
use calc::{eval, eval_polish, CalcError, Value};
use std::io::{self, Write};

const REPL_GUIDE: &str = r#"ion-calc
Type in expressions to have them evaluated.
Type "help" for help."#;

fn calc_or_polish_calc(args: &str) -> Result<Value, CalcError> {
    match eval(args) {
        Ok(t) => Ok(t),
        Err(_) => eval_polish(args),
    }
}

#[builtin(
    desc = "Floating-point calculator",
    man = "
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
    Written by Hunter Goldstein."
)]
pub fn calc(args: &[crate::types::Str], _: &mut crate::Shell<'_>) -> Status {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if args.get(1).is_some() {
        let result = calc_or_polish_calc(&args[1..].join(" "));
        match result {
            Ok(v) => {
                println!("{}", v);
                Status::SUCCESS
            }
            Err(e) => Status::error(format!("{}", e)),
        }
    } else {
        let prompt = b"ion-calc: ";
        println!("{}", REPL_GUIDE);
        loop {
            let _ = stdout.write(prompt);
            let _ = stdout.flush();
            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);
            if input.is_empty() {
                return Status::SUCCESS;
            } else {
                match input.trim() {
                    "" => (),
                    "exit" => return Status::SUCCESS,
                    s => {
                        let result = calc_or_polish_calc(s);
                        match result {
                            Ok(v) => println!("{}", v),
                            Err(e) => eprintln!("{}", e),
                        }
                    }
                }
            }
        }
    }
}
