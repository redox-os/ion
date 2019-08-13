use super::Status;
use crate as ion_shell;
use builtins_proc::builtin;
use calc::{eval_polish_with_env, eval_with_env, CalcError, Value};
use rustyline::Editor;
use std::io::{self, Read};

const REPL_GUIDE: &str = r#"Ion's integrated calculator
Type in expressions to have them evaluated.
Type "help" for help."#;

const REPL_HELP: &str = r#"
Ion-math is a floating-point calculator
You can use infix (ex: 1 + 2 * 3) or polish (+ * 2 3 1) notations.
Non-operator, non-number sequences will be treated as variables for interpolation.

Examples:
    $ 1 + 3-2
    >> 2
    $ 0.00001 + 0.0001
    >> 0.00011

    In Ion if $a = 2, $b = 3, $c = 7
    $ a * b * c
    >> 42
"#;

fn calc_or_polish_calc(args: &str) -> Result<Value, CalcError> {
    let mut env = calc::parse::DefaultEnvironment::new();
    eval_with_env(args, &mut env).or_else(|_| eval_polish_with_env(args, &mut env))
}

fn calc_or_polish_calc_with_env(
    args: &str,
    env: &mut impl calc::parse::Environment,
) -> Result<Value, CalcError> {
    eval_with_env(args, env).or_else(|_| eval_polish_with_env(args, env))
}

#[builtin(
    desc = "Floating-point calculator",
    man = "
SYNOPSIS
    math [EXPRESSION]

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
        math 2+2

    Add two plus two in polish notation
        math + 2 2

AUTHOR
    Written by Hunter Goldstein."
)]
pub fn math(args: &[crate::types::Str], _: &mut crate::Shell<'_>) -> Status {
    if args.get(1).is_some() {
        let result = calc_or_polish_calc(&args[1..].join(" "));
        match result {
            Ok(v) => {
                println!("{}", v);
                Status::SUCCESS
            }
            Err(e) => Status::error(format!("{}", e)),
        }
    } else if atty::is(atty::Stream::Stdin) {
        println!("{}", REPL_GUIDE);
        let mut context = Editor::<()>::new();
        let mut ans = None;
        loop {
            match context.readline("ion-math: ").as_ref().map(AsRef::as_ref) {
                Ok("") => return Status::SUCCESS,
                Ok(text) if text.trim() == "exit" => return Status::SUCCESS,
                Ok(text) if text.trim() == "help" => eprintln!("{}", REPL_HELP),
                Ok(s) => {
                    let mut env = calc::parse::DefaultEnvironment::with_ans(ans.clone());
                    let result = calc_or_polish_calc_with_env(s, &mut env);
                    match result {
                        Ok(v) => {
                            println!("{}", v);
                            ans = Some(v);
                        }
                        Err(e) => eprintln!("{}", e),
                    }
                }
                Err(err) => {
                    eprintln!("{}", err);
                    return Status::SUCCESS;
                }
            }
        }
    } else {
        let mut input = String::with_capacity(1024);
        io::stdin().read_to_string(&mut input).unwrap();

        let result = calc_or_polish_calc(&input);
        match result {
            Ok(v) => {
                println!("{}", v);
                Status::SUCCESS
            }
            Err(e) => Status::error(format!("{}", e)),
        }
    }
}
