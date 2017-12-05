use std::error::Error;
use std::io::{stdout, Write};

pub(crate) fn print_man(man_page: &'static str) {
    let stdout = stdout();
    let mut stdout = stdout.lock();
    match stdout.write_all(man_page.as_bytes()).and_then(|_| stdout.flush()) {
        Ok(_) => (),
        Err(err) => panic!("{}", err.description().to_owned()),
    }
}

pub(crate) fn check_help(args: &[&str], man_page: &'static str) -> bool {
    for arg in args {
        if *arg == "-h" || *arg == "--help" {
            print_man(man_page);
            return true
        }
    }
    false
}

pub(crate) const MAN_STATUS: &'static str = r#"NAME
    status - Evaluates the current runtime status

SYNOPSIS
    status [ -h | --help ] [-l] [-i]

DESCRIPTION
    With no arguments status displays the current login information of the shell.

OPTIONS
    -l
        returns true if the shell is a login shell. Also --is-login.
    -i
        returns true if the shell is interactive. Also --is-interactive.
    -f
        prints the filename of the currently running script or else stdio. Also --current-filename.
"#;

pub(crate) const MAN_CD: &'static str = r#"NAME
    cd - Change directory.

SYNOPSIS
    cd DIRECTORY

DESCRIPTION
    Without arguments cd changes the working directory to your home directory.

    With arguments cd changes the working directory to the directory you provided.

"#;

pub(crate) const MAN_BOOL: &'static str = r#"NAME
    bool - Returns true if the value given to it is equal to '1' or 'true'.

SYNOPSIS
    bool VALUE

DESCRIPTION
    Returns true if the value given to it is equal to '1' or 'true'.
"#;

pub(crate) const MAN_IS: &'static str = r#"NAME
    is - Checks if two arguments are the same

SYNOPSIS
    is [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal.
"#;

pub(crate) const MAN_DIRS: &'static str = r#"NAME
    dirs - prints the directory stack

SYNOPSIS
    dirs

DESCRIPTION
    dirs prints the current directory stack.
"#;

pub(crate) const MAN_PUSHD: &'static str = r#"NAME
    pushd - push a directory to the directory stack

SYNOPSIS
    pushd DIRECTORY

DESCRIPTION
    pushd pushes a directory to the directory stack.
"#;

pub(crate) const MAN_POPD: &'static str = r#"NAME
    popd - shift through the directory stack

SYNOPSIS
    popd

DESCRIPTION
    popd removes the top directory from the directory stack and changes the working directory to the new top directory. 
    pushd adds directories to the stack.
"#;

/*pub(crate) const MAN_FN: &'static str = r#"NAME
    fn - print a list of all functions or create a function

SYNOPSIS
    fn

    fn example arg:int
        echo $arg
    end

DESCRIPTION
    fn prints a list of all functions that exist in the shell or creates a function when combined
    with the 'end' keyword. Functions can have type hints, to tell ion to check the type of a 
    functions arguments. An error will occur if an argument supplied to a function is of the wrong type.
    The supported types in ion are, [], bool, bool[], float, float[], int, int[], str, str[].

    Functions are called by typing the function name and then the function arguments, separated
    by a space.

    fn example arg0:int arg1:int
        echo $arg
    end

    example 1
"#;*/

pub(crate) const MAN_READ: &'static str = r#"NAME
    read - read a line of input into some variables

SYNOPSIS
    read VARIABLES...

DESCRIPTION
    For each variable reads from standard input and stores the results in the variable.
"#;

pub(crate) const MAN_DROP: &'static str = r#"NAME
    drop - delete some variables or arrays

SYNOPSIS
    drop [ -a ] VARIABLES...

DESCRIPTION
    Deletes the variables given to it as arguments. The variables name must be supplied.
    Instead of '$x' use 'x'.

OPTIONS
    -a
        Instead of deleting variables deletes arrays.
"#;

pub(crate) const MAN_SET: &'static str = r#"NAME
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
        If no arguments are suppled, arguments will not be unset.
"#;

pub(crate) const MAN_EVAL: &'static str = r#"NAME
    eval - evaluates the specified commands

SYNOPSIS
    eval COMMANDS...

DESCRIPTION
    eval evaluates the given arguments as a command. If more than one argument is given,
    all arguments are joined using a space as a separator.
"#;

pub(crate) const MAN_HISTORY: &'static str = r#"NAME
    history - print command history

SYNOPSIS
    history

DESCRIPTION
    Prints the command history.
"#;

pub(crate) const MAN_SOURCE: &'static str = r#"NAME
    source - evaluates given file

SYNOPSIS
    source FILEPATH

DESCRIPTION
    Evaluates the commands in a specified file in the current shell. All changes in shell 
    variables will affect the current shell because of this.
"#;

pub(crate) const MAN_ECHO: &'static str = r#"NAME
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
        \v  vertical tab (VT)
"#;

pub(crate) const MAN_TEST: &'static str = r#"NAME
    test - perform tests on files and text

SYNOPSIS
    test [EXPRESSION]

DESCRIPTION
    Tests the expressions given and returns an exit status of 0 if true, else 1.

OPTIONS
    -n STRING
        the length of STRING is nonzero

    STRING
        equivalent to -n STRING

    -z STRING
        the length of STRING is zero

    STRING = STRING
        the strings are equivalent

    STRING != STRING
        the strings are not equal

    INTEGER -eq INTEGER
        the integers are equal

    INTEGER -ge INTEGER
        the first INTEGER is greater than or equal to the first INTEGER

    INTEGER -gt INTEGER
        the first INTEGER is greater than the first INTEGER

    INTEGER -le INTEGER
        the first INTEGER is less than or equal to the first INTEGER

    INTEGER -lt INTEGER
        the first INTEGER is less than the first INTEGER

    INTEGER -ne INTEGER
        the first INTEGER is not equal to the first INTEGER

    FILE -ef FILE
        both files have the same device and inode numbers

    FILE -nt FILE
        the first FILE is newer than the second FILE

    FILE -ot FILE
        the first file is older than the second FILE

    -b FILE
        FILE exists and is a block device

    -c FILE
        FILE exists and is a character device

    -d FILE
        FILE exists and is a directory

    -e FILE
        FILE exists

    -f FILE
        FILE exists and is a regular file

    -h FILE
        FILE exists and is a symbolic link (same as -L)

    -L FILE
        FILE exists and is a symbolic link (same as -h)

    -r FILE
        FILE exists and read permission is granted

    -s FILE
        FILE exists and has a file size greater than zero

    -S FILE
        FILE exists and is a socket

    -w FILE
        FILE exists and write permission is granted

    -x FILE
        FILE exists and execute (or search) permission is granted

EXAMPLES
    Test if the file exists:
        test -e FILE && echo "The FILE exists" || echo "The FILE does not exist"

    Test if the file exists and is a regular file, and if so, write to it:
        test -f FILE && echo "Hello, FILE" >> FILE || echo "Cannot write to a directory"

    Test if 10 is greater than 5:
        test 10 -gt 5 && echo "10 is greater than 5" || echo "10 is not greater than 5"

    Test if the user is running a 64-bit OS (POSIX environment only):
        test $(getconf LONG_BIT) = 64 && echo "64-bit OS" || echo "32-bit OS"

AUTHOR
    Written by Michael Murphy.
"#;

pub(crate) const MAN_RANDOM: &'static str = r#"NAME
    random - generate a random number

SYNOPSIS
    random
    random START END

DESCRIPTION
    random generates a pseudo-random integer. IT IS NOT SECURE.
    The range depends on what arguments you pass. If no arguments are given the range is [0, 32767]. 
    If two arguments are given the range is [START, END].
"#;

pub(crate) const MAN_TRUE: &'static str = r#"NAME
    true - does nothing successfully

SYNOPSIS
    true

DESCRIPTION
    Sets the exit status to 0.
"#;

pub(crate) const MAN_FALSE: &'static str = r#"NAME
    false - does nothing unsuccessfully

SYNOPSIS
    false

DESCRIPTION
    Sets the exit status to 1.
"#;

pub(crate) const MAN_JOBS: &'static str = r#"NAME
    jobs - list all jobs running in the background

SYNOPSIS
    jobs

DESCRIPTION
    Prints a list of all jobs running in the background.
"#;

pub(crate) const MAN_BG: &'static str = r#"NAME
    bg - sends jobs to background

SYNOPSIS
    bg PID

DESCRIPTION
    bg sends the job to the background resuming it if it has stopped.
"#;

pub(crate) const MAN_FG: &'static str = r#"NAME
    fg - bring job to foreground

SYNOPSIS
    fg PID

DESCRIPTION
    fg brings the specified job to foreground resuming it if it has stopped.
"#;

pub(crate) const MAN_SUSPEND: &'static str = r#"NAME
    suspend - suspend the current shell

SYNOPSIS
    suspend

DESCRIPTION
    Suspends the current shell by sending it the SIGTSTP signal, 
    returning to the parent process. It can be resumed by sending it SIGCONT.
"#;

pub(crate) const MAN_DISOWN: &'static str = r#"NAME
    disown - Disown processes

SYNOPSIS
    disown [ --help | -r | -h | -a ][PID...]

DESCRIPTION
    Disowning a process removes that process from the shell's background process table.

OPTIONS
    -r  Remove all running jobs from the background process list.
    -h  Specifies that each job supplied will not receive the SIGHUP signal when the shell receives a SIGHUP.
    -a  If no job IDs were supplied, remove all jobs from the background process list.
"#;

pub(crate) const MAN_EXIT: &'static str = r#"NAME
    exit - exit the shell

SYNOPSIS
    exit

DESCRIPTION
    Makes ion exit. The exit status will be that of the last command executed.
"#;

pub(crate) const MAN_MATCHES: &'static str = r#"NAME
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
        matches x xs
"#;

pub(crate) const MAN_NOT: &'static str = r#"NAME
    not - reverses the exit status of a job

SYNOPSIS
    not

DESCRIPTION
    not reverses the exit status of a job. If the exit status is 1 not returns 0 and vice versa.
"#;

pub(crate) const MAN_AND: &'static str = r#"NAME
    and - check if to execute another command and a boolean gate.

SYNOPSIS
    COMMAND and COMMAND
    COMMAND; and COMMAND

DESCRIPTION
    and changes the exit status to 0 if the previous command and the next command are true. 
    and can also be used to execute multiple commands if they all are successful. '&&' is preferred to 'and'
    in if, while and all other similar conditional statements.  

EXAMPLES
    Returns an exit of status 0:
        true and true
    Returns an exit of status 1:
        true and false

    Executes all the commands:
        echo "1"; and echo "2"
    Executes no commands:
        false; and echo "1"
"#;

pub(crate) const MAN_OR: &'static str = r#"NAME
    or - conditionally run a command

SYNOPSIS
    COMMAND; or COMMAND

DESCRIPTION
    or can be used to execute a command if the exit status of the previous command is not 0.
    or can also be used in if, while and other similar statements, however '||' is preferred.

EXAMPLE
    Executes all of the code block, prints 2:
        false; or echo "2" 
    Does not execute all of the code block, prints 1:
        echo "1"; or echo "2"
"#;

pub(crate) const MAN_EXISTS: &'static str = r#"NAME
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
    Heavily based on implementation of the test builtin, which was written by Michael Murph.
"#;

pub(crate) const MAN_WHICH: &'static str = r#"NAME
    which - locate a program file in the current user's path

SYNOPSIS
    which PROGRAM 

DESCRIPTION
    The which utility takes a list of command names and searches for the 
    alias/builtin/function/executable that whould be executed if you ran that command.
"#;