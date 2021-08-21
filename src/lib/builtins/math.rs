use super::{EmptyCompleter, Status};
use crate as ion_shell;
use builtins_proc::builtin;
use calc::{eval_polish_with_env, eval_with_env, CalcError, Value};
use liner::{Context, Prompt};
use std::io::{self, Read};

const REPL_NO_TTY_INIT_CAPACITY: usize = 1024;

const QUIET_FLAG: &str = "-q";

const REPL_WELCOME: &str = r#"Ion's integrated calculator
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

fn repl() -> Status {
    let mut context = Context::new();
    let mut ans = None;
    loop {
        match context
            .read_line(Prompt::from("ion-math: "), None, &mut EmptyCompleter)
            .as_ref()
            .map(AsRef::as_ref)
        {
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
}

fn init_repl(flag: Option<&str>) -> Status {
    if atty::is(atty::Stream::Stdin) {
        if let Some(QUIET_FLAG) = flag {
            repl()
        } else {
            println!("{}", REPL_WELCOME);
            repl()
        }
    } else {
        let mut input = String::with_capacity(REPL_NO_TTY_INIT_CAPACITY);
        io::stdin().read_to_string(&mut input).unwrap();
        repl()
    }
}

#[builtin(
    desc = "Floating-point calculator",
    man = "
SYNOPSIS
    math [EXPRESSION]

DESCRIPTION
    Evaluates arithmetic expressions

SPECIAL EXPRESSIONS
    -q (only in non-interactive mode)
        opens interactive mode without welcome message

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
    match args.get(1) {
        Some(s) if s == "-q" => init_repl(Some(&s)),
        Some(_) => {
            let result = calc_or_polish_calc(&args[1..].join(" "));
            match result {
                Ok(v) => {
                    println!("{}", v);
                    Status::SUCCESS
                }
                Err(e) => Status::error(format!("{}", e)),
            }
        }
        None => init_repl(None),
    }
}
