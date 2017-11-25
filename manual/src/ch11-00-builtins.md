# Builtin Commands

## alias

```
alias NAME=DEFINITION
alias NAME DEFINITION
```

View, set or unset aliases

## and

```
COMMAND; and COMMAND
```

Execute the command if the shell's previous status is success

## bg

```
bg [PID]
```

Resumes a stopped background process. If no process is specified, the previous
job will resume.

## calc

```
calc [EXPRESSION]
```

Calculate a mathematical expression. If no expression is given, it will open
an interactive expression engine. Type exit to leave the engine.

## cd

```
cd [PATH]
```

Change the current directory and push it to the stack.
Omit the directory to change to home

## contains

```
contains KEY [VALUE...]
```

Evaluates if the supplied argument contains a given string

## dirs

```
dirs
```

Display the current directory stack

## disown

```
disown [-r | -h | -a ][PID...]
```

Disowning a process removes that process from the shell's background process table.
If no process is specified, the most recently-used job is removed

## drop

```
drop VARIABLE
drop -a ARRAY_VARIABLE
```

Drops a variable from the shell's variable map. By default, this will drop string variables from
the string variable map. If the `-a` flag is specified, array variables will be dropped from the
array variable map instead.

## echo

```
echo [ -h | --help ] [-e] [-n] [-s] [STRING]...
```

Display a line of text

#### Options

- **-e**: enable the interpretation of backslash escapes
- **-n**: do not output the trailing newline
- **-s**: do not separate arguments with spaces

#### Escape Sequences

When the -e argument is used, the following sequences will be interpreted:

- **\\**: backslash
- **\a**: alert (BEL)
- **\b**: backspace (BS)
- **\c**: produce no further output
- **\e**: escape (ESC)
- **\f**: form feed (FF)
- **\n**: new line
- **\r**: carriage return
- **\t**: horizontal tab (HT)
- **\v**: vertical tab (VT)

## ends-with

```
ends-with KEY [VALUE...]
```

Evaluates if the supplied argument ends with a given string

## eval

```
eval COMMAND
```

evaluates the evaluated expression

## exists

```
exists [-a ARRAY] [-b BINARY] [-d PATH] [--fn FUNCTION] [[-s] STRING]
```

Performs tests on files and text

#### options

- **-a ARRAY**:      array var is not empty
- **-b BINARY**:     binary is in PATH
- **-d PATH**:       path is a directory
- **-f PATH**:       path is a file
- **--fn FUNCTION**: function is defined
- **-s STRING**:     string var is not empty
- **STRING**:        string is not empty

## exit

```
exit
```

Exits the current session and kills all background tasks

## false

```
false
```

Do nothing, unsuccessfully

## fg

```
fg [PID]
```

Resumes and sets a background process as the active process. If no process is specified, the previous job will be the active process.

## fn

```
fn
```

Print list of functions

## help

```
help COMMAND
```

Display helpful information about a given command or list commands if
none specified

## history

```
history
````

Display a log of all commands previously executed

## ion-docs

```
ion_docs
```

Opens the Ion manual

## jobs

```
jobs
```

Displays all jobs that are attached to the background

## matches

```
matches VARIABLE REGEX
```

Checks if a string matches a given regex

## not

```
not COMMAND
```
Reverses the exit status value of the given command.

## or

```
COMMAND; or COMMAND
```

Execute the command if the shell's previous status is failure

## popd

```
popd
```

Pop a directory from the stack and returns to the previous directory

## pushd

```
pushd DIRECTORY
```
Push a directory to the stack.

## random

```
random
random SEED
random START END
random START STEP END
random choice [ITEMS...]
```

RANDOM generates a pseudo-random integer from a uniform distribution. The range (inclusive) is
dependent on the arguments passed. No arguments indicate a range of [0; 32767]. If one argument
is specified, the internal engine will be seeded with the argument for future invocations of
RANDOM and no output will be produced. Two arguments indicate a range of [START; END]. Three
arguments indicate a range of [START; END] with a spacing of STEP between possible outputs.
RANDOM choice will select one random item from the succeeding arguments.

> Due to limitations int the rand crate, seeding is not yet implemented

## read

```
read VARIABLE

```
Read some variables

## set

```
set [ --help ] [-e | +e] [-x | +x] [-o [vi | emacs]] [- | --] [STRING]...
```

Set or unset values of shell options and positional parameters.
Shell options may be set using the '-' character,
and unset using the '+' character.

### OPTIONS

- **e**: Exit immediately if a command exits with a non-zero status.

- **-o**: Specifies that an argument will follow that sets the key map.
    - The keymap argument may be either **vi** or **emacs**.

- **-x**: Specifies that commands will be printed as they are executed.

- **--**: Following arguments will be set as positional arguments in the shell.
    - If no argument are supplied, arguments will be unset.

- **-**: Following arguments will be set as positional arguments in the shell.
    - If no arguments are suppled, arguments will not be unset.

## source

```
source [PATH]
```

Evaluate the file following the command or re-initialize the init file

## starts-with

```
ends-with KEY [VALUE...]
```

Evaluates if the supplied argument starts with a given string

## suspend

```
suspend
```

Suspends the shell with a SIGTSTOP signal

## test

```
test [EXPRESSION]
```

Performs tests on files and text

#### Options

- **-n STRING**:         the length of STRING is nonzero  
- **STRING**:            equivalent to -n STRING  
- **-z STRING**:         the length of STRING is zero  
- **STRING = STRING**:   the strings are equivalent  
- **STRING != STRING**:  the strings are not equal  
- **INTEGER -eq INTEGER**: the integers are equal  
- **INTEGER -ge INTEGER**: the first INTEGER is greater than or equal to the first INTEGER  
- **INTEGER -gt INTEGER**: the first INTEGER is greater than the first INTEGER  
- **INTEGER -le INTEGER**: the first INTEGER is less than or equal to the first INTEGER  
- **INTEGER -lt INTEGER**: the first INTEGER is less than the first INTEGER  
- **INTEGER -ne INTEGER**: the first INTEGER is not equal to the first INTEGER  
- **FILE -ef FILE**:     both files have the same device and inode numbers  
- **FILE -nt FILE**:     the first FILE is newer than the second FILE  
- **FILE -ot FILE**:     the first file is older than the second FILE  
- **-b FILE**:          FILE exists and is a block device  
- **-c FILE**:           FILE exists and is a character device  
- **-d FILE**:           FILE exists and is a directory  
- **-e FILE**:           FILE exists  
- **-f FILE**:           FILE exists and is a regular file  
- **-h FILE**:           FILE exists and is a symbolic link (same as -L)  
- **-L FILE**:           FILE exists and is a symbolic link (same as -h)  
- **-r FILE**:           FILE exists and read permission is granted  
- **-s FILE**:           FILE exists and has a file size greater than zero  
- **-S FILE**:           FILE exists and is a socket  
- **-w FILE**:           FILE exists and write permission is granted  
- **-x FILE**:           FILE exists and execute (or search) permission is granted  

## true

```
true
```

Do nothing, successfully

## unalias

```
unalias VARIABLE...
```

Delete an alias

## wait

```
wait
```

Waits until all running background processes have completed

## which

```
which COMMAND
```

Shows the full path of commands

## status

```
status COMMAND
```

Evaluates the current runtime status

### Options

- **-l**: returns true if shell is a login shell
- **-i**: returns true if shell is interactive
- **-f**: prints the filename of the currently running script or stdio