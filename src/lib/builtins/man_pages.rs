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

pub const MAN_READ: &str = r#"NAME
    read - read a line of input into some variables

SYNOPSIS
    read VARIABLES...

DESCRIPTION
    For each variable reads from standard input and stores the results in the variable."#;

pub const MAN_DROP: &str = r#"NAME
    drop - delete some variables or arrays

SYNOPSIS
    drop [ -a ] VARIABLES...

DESCRIPTION
    Deletes the variables given to it as arguments. The variables name must be supplied.
    Instead of '$x' use 'x'.

OPTIONS
    -a
        Instead of deleting variables deletes arrays."#;

pub const MAN_SET: &str = r#"NAME
    set - Set or unset values of shell options and positional parameters.

SYNOPSIS
    set [ --help ] [-e | +e] [-x | +x] [-o [vi | emacs]] [- | --] [STRING]...

DESCRIPTION
    Shell options may be set using the '-' character, and unset using the '+' character.

OPTIONS
    -e  Exit immediately if a command exits with a non-zero status.

    -o  Specifies that an argument will follow that sets the key map.
        The keymap argument may be either `vi` or `emacs`.

    -x  Specifies that commands will be printed as they are executed.

    --  Following arguments will be set as positional arguments in the shell.
        If no argument are supplied, arguments will be unset.

    -   Following arguments will be set as positional arguments in the shell.
        If no arguments are suppled, arguments will not be unset."#;

pub const MAN_EQ: &str = r#"NAME
    eq - Checks if two arguments are the same

SYNOPSIS
    eq [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal."#;

pub const MAN_EVAL: &str = r#"NAME
    eval - evaluates the specified commands

SYNOPSIS
    eval COMMANDS...

DESCRIPTION
    eval evaluates the given arguments as a command. If more than one argument is given,
    all arguments are joined using a space as a separator."#;

pub const MAN_SOURCE: &str = r#"NAME
    source - evaluates given file

SYNOPSIS
    source FILEPATH

DESCRIPTION
    Evaluates the commands in a specified file in the current shell. All changes in shell
    variables will affect the current shell because of this."#;

pub const MAN_ECHO: &str = r#"NAME
    echo - display a line of text

SYNOPSIS
    echo [ -h | --help ] [-e] [-n] [-s] [STRING]...

DESCRIPTION
    Print the STRING(s) to standard output.

OPTIONS
    -e
        enable the interpretation of backslash escapes
    -n
        do not output the trailing newline
    -s
        do not separate arguments with spaces

    Escape Sequences
        When the -e argument is used, the following sequences will be interpreted:
        \\  backslash
        \a  alert (BEL)
        \b  backspace (BS)
        \c  produce no further output
        \e  escape (ESC)
        \f  form feed (FF)
        \n  new line
        \r  carriage return
        \t  horizontal tab (HT)
        \v  vertical tab (VT)"#;

pub const MAN_RANDOM: &str = r#"NAME
    random - generate a random number

SYNOPSIS
    random
    random START END

DESCRIPTION
    random generates a pseudo-random integer. IT IS NOT SECURE.
    The range depends on what arguments you pass. If no arguments are given the range is [0, 32767].
    If two arguments are given the range is [START, END]."#;

pub const MAN_TRUE: &str = r#"NAME
    true - does nothing successfully

SYNOPSIS
    true

DESCRIPTION
    Sets the exit status to 0."#;

pub const MAN_FALSE: &str = r#"NAME
    false - does nothing unsuccessfully

SYNOPSIS
    false

DESCRIPTION
    Sets the exit status to 1."#;

pub const MAN_JOBS: &str = r#"NAME
    jobs - list all jobs running in the background

SYNOPSIS
    jobs

DESCRIPTION
    Prints a list of all jobs running in the background."#;

pub const MAN_BG: &str = r#"NAME
    bg - sends jobs to background

SYNOPSIS
    bg PID

DESCRIPTION
    bg sends the job to the background resuming it if it has stopped."#;

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
