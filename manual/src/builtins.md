# Builtin commands

## bg - sends jobs to background

```txt
SYNOPSIS
    bg PID

DESCRIPTION
    bg sends the job to the background resuming it if it has stopped.
```

## bool - Returns true if the value given to it is equal to '1' or 'true'.

```txt
SYNOPSIS
    bool VALUE

DESCRIPTION
    Returns true if the value given to it is equal to '1' or 'true'.
```

## calc - Floating-point calculator

```txt
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
```

## cd - Change directory.

```txt
SYNOPSIS
    cd DIRECTORY

DESCRIPTION
    Without arguments cd changes the working directory to your home directory.
    With arguments cd changes the working directory to the directory you provided.
```

## contains - check if a given string starts with another one

```txt
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0
```

## dir_depth - set the dir stack depth

```txt
SYNOPSYS
    dir_depth [DEPTH]

DESCRIPTION
    If DEPTH is given, set the dir stack max depth to DEPTH, else remove the limit
```

## dirs - prints the directory stack

```txt
SYNOPSIS
    dirs

DESCRIPTION
    dirs prints the current directory stack.
```

## disown - disown processes

```txt
SYNOPSIS
    disown [ --help | -r | -h | -a ][PID...]

DESCRIPTION
    Disowning a process removes that process from the shell's background process table.

OPTIONS
    -r  Remove all running jobs from the background process list.
    -h  Specifies that each job supplied will not receive the SIGHUP signal when the shell receives a SIGHUP.
    -a  If no job IDs were supplied, remove all jobs from the background process list.
```

## drop - delete some variables or arrays

```txt
SYNOPSIS
    drop VARIABLES...

DESCRIPTION
    Deletes the variables given to it as arguments. The variables name must be supplied.
    Instead of '$x' use 'x'.
```

## echo - display text

```txt
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
```

## ends_with - check if a given string starts with another one

```txt
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0
```

## eval - evaluates the specified commands

```txt
SYNOPSIS
    eval COMMANDS...

DESCRIPTION
    eval evaluates the given arguments as a command. If more than one argument is given,
    all arguments are joined using a space as a separator.
```

## exec - replace the shell with the given command

```txt
SYNOPSIS
    exec [-ch] [--help] [command [arguments ...]]

DESCRIPTION
    Execute <command>, replacing the shell with the specified program.
    The <arguments> following the command become the arguments to
    <command>.

OPTIONS
    -c  Execute command with an empty environment.
```

## exists - check whether items exist

```txt
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
        exists -f FILE && echo 'The FILE exists' || echo 'The FILE does not exist'

    Test if some-command exists in the path and is executable:
        exists -b some-command && echo 'some-command exists' || echo 'some-command does not exist'

    Test if variable exists AND is not empty
        exists -s myVar && echo "myVar exists: $myVar" || echo 'myVar does not exist or is empty'
        NOTE: Don't use the '$' sigil, but only the name of the variable to check

    Test if array exists and is not empty
        exists -a myArr && echo "myArr exists: @myArr" || echo 'myArr does not exist or is empty'
        NOTE: Don't use the '@' sigil, but only the name of the array to check

    Test if a function named 'myFunc' exists
        exists --fn myFunc && myFunc || echo 'No function with name myFunc found'

AUTHOR
    Written by Fabian Würfl.
    Heavily based on implementation of the test builtin, which was written by Michael Murphy.
```

## exit - exit the shell

```txt
SYNOPSIS
    exit

DESCRIPTION
    Makes ion exit. The exit status will be that of the last command executed.
```

## false - does nothing unsuccessfully

```txt
SYNOPSIS
    false

DESCRIPTION
    Sets the exit status to 1.
```

## fg - bring job to the foreground

```txt
SYNOPSIS
    fg PID

DESCRIPTION
    fg brings the specified job to foreground resuming it if it has stopped.
```

## fn - print a short description of every defined function

```txt
SYNOPSIS
    fn [ -h | --help ]

DESCRIPTION
    Prints all the defined functions along with their help, if provided
```

## help - get help for builtins

```txt
SYNOPSIS
    help [BUILTIN]

DESCRIPTION
    Get the short description for BUILTIN. If no argument is provided, list all the builtins
```

## eq, is - checks if two arguments are the same

```txt
SYNOPSIS
    is [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal.
```

## isatty - checks if the provided file descriptor is a tty

```txt
SYNOPSIS
    isatty [FD]

DESCRIPTION
    Returns 0 exit status if the supplied file descriptor is a tty.
```

## jobs - list all jobs running in the background

```txt
SYNOPSIS
    jobs

DESCRIPTION
    Prints a list of all jobs running in the background.
```

## matches - checks if the second argument contains any proportion of the first

```txt
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
```

## popd - shift through the directory stack

```txt
SYNOPSIS
    popd

DESCRIPTION
    popd removes the top directory from the directory stack and changes the working directory to the new top directory.
    pushd adds directories to the stack.
```

## pushd - push a directory to the directory stack

```txt
SYNOPSIS
    pushd DIRECTORY

DESCRIPTION
    pushd pushes a directory to the directory stack.
```

## random - generate a random number

```txt
SYNOPSIS
    random
    random START END

DESCRIPTION
    random generates a pseudo-random integer. IT IS NOT SECURE.
    The range depends on what arguments you pass. If no arguments are given the range is [0, 32767].
    If two arguments are given the range is [START, END].
```

## read - read a line of input into some variables

```txt
SYNOPSIS
    read VARIABLES...

DESCRIPTION
    For each variable reads from standard input and stores the results in the variable.
```

## set - Set or unset values of shell options and positional parameters.

```txt
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
```

## source - evaluates given file

```txt
SYNOPSIS
    source FILEPATH

DESCRIPTION
    Evaluates the commands in a specified file in the current shell. All changes in shell
    variables will affect the current shell because of this.
```

## starts_with - check if a given string starts with another one

```txt
SYNOPSIS
    starts_with <PATTERN> tests...

DESCRIPTION
    Returns 0 if any argument starts_with contains the first argument, else returns 0
```

## status - Evaluates the current runtime status

```txt
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
```

## suspend - suspend the current shell

```txt
SYNOPSIS
    suspend

DESCRIPTION
    Suspends the current shell by sending it the SIGTSTP signal,
    returning to the parent process. It can be resumed by sending it SIGCONT.
```

## test - perform tests on files and text

```txt
SYNOPSIS
    test [EXPRESSION]

DESCRIPTION
    Tests the expressions given and returns an exit status of 0 if true, else 1.

OPTIONS
    --help
        prints this help text

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
```

## true - does nothing sucessfully

```txt
SYNOPSIS
    true

DESCRIPTION
    Sets the exit status to 0.
```

## wait - wait for a background job

```txt
SYNOPSIS
    wait

DESCRIPTION
    Wait for the background jobs to finish
```

## which, type - locate a program file in the current user's path

```txt
SYNOPSIS
    which PROGRAM

DESCRIPTION
    The which utility takes a list of command names and searches for the
    alias/builtin/function/executable that would be executed if you ran that command.
```