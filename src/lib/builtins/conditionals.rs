use super::Status;
use crate as ion_shell;
use builtins_proc::builtin;

macro_rules! string_function {
    (#[$outer:meta], $method:tt) => {
        #[$outer]
        pub fn $method(args: &[small::String], _shell: &mut crate::Shell<'_>) -> Status {
            if args.len() <= 2 {
                return Status::bad_argument(concat!(
                    "ion: ",
                    stringify!($method),
                    ": two arguments must be supplied",
                ));
            }
            args[2..].iter().any(|arg| args[1].$method(arg.as_str())).into()
        }
    };
}

string_function!(
#[builtin(
    desc = "check if a given string starts with another one",
    man = "
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0"
)], starts_with);
string_function!(
#[builtin(
    desc = "check if a given string starts with another one",
    man = "
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0"
)], ends_with);
string_function!(
#[builtin(
    desc = "check if a given string starts with another one",
    man = "
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0"
)], contains);
