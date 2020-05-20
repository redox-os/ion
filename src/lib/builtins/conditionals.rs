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
    starts-with <PATTERN> tests...

DESCRIPTION
    Returns 0 if the first argument starts with any other argument, else returns 0"
)], starts_with);
string_function!(
#[builtin(
    desc = "check if a given string ends with another one",
    man = "
SYNOPSIS
    ends-with <PATTERN> tests...

DESCRIPTION
    Returns 0 if the first argument ends with any other argument, else returns 0"
)], ends_with);
string_function!(
#[builtin(
    desc = "check if a given string contains another one",
    man = "
SYNOPSIS
    contains <PATTERN> tests...

DESCRIPTION
    Returns 0 if the first argument contains any other argument, else returns 0"
)], contains);
