use crate::types;

pub fn check_help(args: &[types::Str], man_page: &'static str) -> bool {
    for arg in args {
        if arg == "-h" || arg == "--help" {
            println!("{}", man_page);
            return true;
        }
    }
    false
}

pub const MAN_IS: &str = r#"NAME
    is - Checks if two arguments are the same

SYNOPSIS
    is [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal."#;

pub const MAN_ISATTY: &str = r#"
    isatty - Checks if argument is a file descriptor

SYNOPSIS
    isatty [FD]

DESCRIPTION
    Returns 0 exit status if the supplied file descriptor is a tty."#;

// pub const MAN_FN: &str = r#"NAME
// fn - print a list of all functions or create a function
//
// SYNOPSIS
// fn
//
// fn example arg:int
// echo $arg
// end
//
// DESCRIPTION
// fn prints a list of all functions that exist in the shell or creates a
// function when combined with the 'end' keyword. Functions can have type
// hints, to tell ion to check the type of a functions arguments. An error will
// occur if an argument supplied to a function is of the wrong type.
// The supported types in ion are, [], bool, bool[], float, float[], int,
// int[], str, str[].
//
// Functions are called by typing the function name and then the function
// arguments, separated by a space.
//
// fn example arg0:int arg1:int
// echo $arg
// end
//
// example 1
//"#;

pub const MAN_RANDOM: &str = r#"NAME
    random - generate a random number

SYNOPSIS
    random
    random START END

DESCRIPTION
    random generates a pseudo-random integer. IT IS NOT SECURE.
    The range depends on what arguments you pass. If no arguments are given the range is [0, 32767].
    If two arguments are given the range is [START, END]."#;

pub const MAN_FG: &str = r#"NAME
    fg - bring job to foreground

SYNOPSIS
    fg PID

DESCRIPTION
    fg brings the specified job to foreground resuming it if it has stopped."#;

pub const MAN_SUSPEND: &str = r#"NAME
    suspend - suspend the current shell

SYNOPSIS
    suspend

DESCRIPTION
    Suspends the current shell by sending it the SIGTSTP signal,
    returning to the parent process. It can be resumed by sending it SIGCONT."#;

pub const MAN_DISOWN: &str = r#"NAME
    disown - Disown processes

SYNOPSIS
    disown [ --help | -r | -h | -a ][PID...]

DESCRIPTION
    Disowning a process removes that process from the shell's background process table.

OPTIONS
    -r  Remove all running jobs from the background process list.
    -h  Specifies that each job supplied will not receive the SIGHUP signal when the shell receives a SIGHUP.
    -a  If no job IDs were supplied, remove all jobs from the background process list."#;

pub const MAN_MATCHES: &str = r#"NAME
    matches - checks if the second argument contains any portion of the first.

SYNOPSIS
    matches VALUE VALUE

DESCRIPTION
    Makes the exit status equal 0 if the first argument contains the second.
    Otherwise matches makes the exit status equal 1.

EXAMPLES
    Returns true:
        matches xs x
    Returns false:
        matches x xs"#;

pub const MAN_EXISTS: &str = r#"NAME
    exists - check whether items exist

SYNOPSIS
    exists [EXPRESSION]

DESCRIPTION
    Checks whether the given item exists and returns an exit status of 0 if it does, else 1.

OPTIONS
    -a ARRAY
        array var is not empty

    -b BINARY
        binary is in PATH

    -d PATH
        path is a directory
        This is the same as test -d

    -f PATH
        path is a file
        This is the same as test -f

    --fn FUNCTION
        function is defined

    -s STRING
        string var is not empty

    STRING
        string is not empty
        This is the same as test -n

EXAMPLES
    Test if the file exists:
        exists -f FILE && echo "The FILE exists" || echo "The FILE does not exist"

    Test if some-command exists in the path and is executable:
        exists -b some-command && echo "some-command exists" || echo "some-command does not exist"

    Test if variable exists AND is not empty
        exists -s myVar && echo "myVar exists: $myVar" || echo "myVar does not exist or is empty"
        NOTE: Don't use the '$' sigil, but only the name of the variable to check

    Test if array exists and is not empty
        exists -a myArr && echo "myArr exists: @myArr" || echo "myArr does not exist or is empty"
        NOTE: Don't use the '@' sigil, but only the name of the array to check

    Test if a function named 'myFunc' exists
        exists --fn myFunc && myFunc || echo "No function with name myFunc found"

AUTHOR
    Written by Fabian Würfl.
    Heavily based on implementation of the test builtin, which was written by Michael Murph."#;

pub const MAN_WHICH: &str = r#"NAME
    which - locate a program file in the current user's path

SYNOPSIS
    which PROGRAM

DESCRIPTION
    The which utility takes a list of command names and searches for the
    alias/builtin/function/executable that would be executed if you ran that command."#;
