use super::{EmptyCompleter, Status};
use crate as ion_shell;
use builtins_proc::builtin;
use calc::{eval, eval_polish, CalcError, Value};
use liner::Context;

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
        println!("{}", REPL_GUIDE);
        let mut context = Context::new();
        loop {
            match context
                .read_line("ion-calc: ", None, &mut EmptyCompleter)
                .as_ref()
                .map(AsRef::as_ref)
            {
                Ok("") => return Status::SUCCESS,
                Ok(text) if text.trim() == "exit" => return Status::SUCCESS,
                Ok(s) => {
                    let result = calc_or_polish_calc(s);
                    match result {
                        Ok(v) => println!("{}", v),
                        Err(e) => eprintln!("{}", e),
                    }
                }
                Err(err) => {
                    eprintln!("{}", err);
                    return Status::SUCCESS;
                }
            }
        }
    }
}
