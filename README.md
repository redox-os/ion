# Ion Shell

[![Build Status](https://travis-ci.org/redox-os/ion.svg)](https://travis-ci.org/redox-os/ion)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Coverage Status](https://coveralls.io/repos/redox-os/ion/badge.svg?branch=master&service=github)](https://coveralls.io/github/redox-os/ion?branch=master)
[![crates.io](http://meritbadge.herokuapp.com/ion-shell)](https://crates.io/crates/ion-shell)

Ion is a shell for UNIX platforms, and is the default shell in Redox. It is still a work in progress, but much of the core functionality is complete. It is also currently significantly faster than Bash, and even Dash, making it the fastest system shell to date.

# Completed Features

- [x] Variable Expansions
- [x] Brace Expansions
- [x] Process Expansions
- [x] Flow Control
- [x] For Loops
- [x] While Loops
- [x] If Conditionals
- [x] Functions
- [x] Optionally-Typed Function Parameters
- [x] Executing Scripts with an @args Array
- [x] Aliases
- [x] Variables (**$variable**)
- [x] Multiple Variable Assignments
- [x] Substring Slicing of Variables
- [x] Arrays (**@array**)
- [x] Array Expressions (**[]**)
- [x] Array-based Command Substitution (**@[]**)
- [x] String-based Command Substitution (**$()**)
- [x] Array Methods (**@split(var, ' ')**)
- [x] String Methods (**$join(array, ', ')**)
- [x] Array Splicing
- [x] Piping Stdout/Stderr
- [x] Redirecting Stdout/Stderr
- [x] Piping Builtins
- [x] **&&** and **||** Conditionals
- [x] Background Jobs
- [x] Multiline Comments and Commands
- [x] Tab Completion (Needs Improvements)
- [x] vi and emacs keybindings (`set -o (vi|emacs)`)
- [x] Implicit cd

## Unimplemented Features

Currently, the most important missing feature is support for signal handling, which is not well supported by in Rust at this time due to the lack of developed signal handling crates, and Redox not having support for signal handling.

- [ ] Signal Handling
- [ ] Multiline Editing
- [ ] XDG App Dirs
- [ ] Background Jobs Control
- [ ] Autosuggestions (90%)
- [ ] Syntax Highlighting
- [ ] Piping Functions
- [ ] Maps
- [ ] Lists?
- [ ] Foreach Loops
- [ ] Syntax for Color Handling
- [ ] Builtin Plugins
- [ ] Prompt Plugins
- [ ] Syntax Plugins


## Shell Syntax

### Implicit Directory Changing

Like the [Friendly Interactive Shell](https://fishshell.com/), Ion also supports implicitly executing the cd command when given
a path, so long as that path begins with either `.`, `/`, or `~`; or ends with a `/`. This will thereby invoke
the internal built-in cd command with that path as the argument.

```ion
~/Documents # cd ~/Documents
..          # cd ..
.config     # cd .config
examples/   # cd examples/
```

### Defining Variables

The `let` keyword is utilized to create local variables within the shell. The `export` keyword performs
a similar action, only setting the variable globally as an environment variable for the operating system.

```ion
let git_branch = $(git rev-parse --abbrev-ref HEAD ^> /dev/null)
```

It is also possible to assign multiple variables at once, or swap variables.

```ion
let a b = 1 2
let a b = [1 2]
let a b = [$b $a]
```

If the command is executed without any arguments, it will simply list all available variables.

### Using Variables

Variables may be called with the **$** sigil, where the value that follows may be a local or global value.
They may also be optionally defined using a braced syntax, which is useful in the event that you need the value
integrated alongside other characters that do not terminate the variable parsing.

```ion
let A = one
let B = two
echo $A:$B
echo ${A}s and ${B}s
```

### Substrings from Variables

Ion natively supports splitting supplied strings by graphemes using the same slicing syntax for arrays:

```ion
$ let string = "one two three"
$ echo $string[0]
o
$ echo $string[..3]
one
$ echo $string[4..7]
two
$ echo $string[8..]
three
```

### Dropping Variables

To drop a value from the shell, the `drop` keyword may be used:

```ion
drop git_branch
```

### Variable Arithmetic

The `let` command also supports basic arithmetic.

```ion
let a = 1
echo $a
let a += 4
echo $a
let a *= 10
echo $a
let a /= 2
echo $a
let a -= 5
echo $a
```

### Export

The `export` command works similarly to the `let` command, but instead of defining a local variable, it defines a
global variable that other processes can access.

```ion
export PATH = "~/.cargo/bin:${PATH}"
```

### Export Arithmetic

The `export` command also supports basic arithmetic.

```ion
export a = 1
echo $a
export a += 4
echo $a
export a *= 10
echo $a
export a /= 2
echo $a
export a -= 5
echo $a
```

### Aliases

The `alias` command is used to set an alias for running other commands under a different name. The most common usages of the `alias` keyword are to shorten the keystrokes required to run a command and it's specific arguments, and to rename a command to something more familiar.

```ion
alias ls = 'exa'
```

If the command is executed without any arguments, it will simply list all available aliases.

The `unalias` command performs the reverse of `alias` in that it drops the value from existence.

```ion
unalias ls
```

### Brace Expansion

Brace expansions are used to create permutations of a given input. In addition to simple permutations, Ion supports
brace ranges and nested branches.

```ion
echo abc{3..1}def{1..3,a..c}
echo ghi{one{a,b,c},two{d,e,f}}
```

### Defining Arrays

Arrays can be create with the let keyword when the supplied expression evaluates to a vector of values:

#### Array Syntax

The basic syntax for creating an array of values is to wrap the values inbetween **[]** characters. The syntax within
will be evaluated into a flat-mapped vector, and the result can therefor be stored as an array.

```ion
let array = [ one two 'three four' ]
```

One particular use case for arrays is setting command arguments

```ion
let lsflags = [ -l -a ]
ls @lsflags
```

#### Braces Create Arrays

Brace expansions actually create a vector of values under the hood, and thus they can be used to create an array.

```ion
let braced_array = {down,up}vote
```

#### Array-based Command Substitution

Whereas the standard command substitution syntax will create a single string from the output, this variant will create
a whitespace-delimited vector of values from the output of the command.

```ion
let word_split_process = @[echo one two three]
```

### Using Arrays

Arrays may be called with the **@** sigil, which works identical to the variable syntax:

```ion
echo @braced_array
echo @{braced_array}
```

Arrays may also be sliced when an index or index range is supplied:

#### Slice by Index

Slicing by an index will take a string from an array:

```ion
let array = [ 1 2 3 ]
echo @array[0]
echo @array[1]
echo @array[2]

echo [ 1 2 3 ][0]
echo [ 1 2 3 ][1]
echo [ 1 2 3 ][2]

echo @[echo 1 2 3][0]
echo @[echo 1 2 3][1]
echo @[echo 1 2 3][2]
```

#### Slice by Range

Slicing by range will take a subsection of an array as a new array:

```ion
let array = [ 1 2 3 4 5 ]
echo @array[0..1]
echo @array[0...1]
echo @array[..3]
echo @array[3..]
echo @array[..]
```

### Methods

There are two types of methods -- string-based and array-based methods. The type that a method returns is denoted
by the sigil that is used to invoke the method. Currently, there are only two supported methods: **$join()** and
**@split**.

```ion
let results = [ 1 2 3 4 5]
echo $join(results) @join # Both of these effectively do the same thing
echo $join(results, ', ') # You may provide a custom pattern instead

let line = "one  two  three  four  five"
echo @split(line) # Splits a line by whitespace

let row = "one,two,three,four,five"
echo @split(row, ',') # Splits by commas
```

### Substring Slicing on String Methods

```ion
echo $join(array)[3..6]
```

### Array Slicing on Array Methods

```ion
let cpu_model = $(grep "model name" /proc/cpuinfo | head -1)
echo @split(cpu_model)[3..5]
```

### Commands

Commands may be written line by line or altogether on the same line with semicolons separating them.

```ion
command arg1 arg2 arg3
command arg1 arg2 arg3
command arg1 arg2 arg3; command arg1 arg2 arg3; command arg1 arg2 arg3
```

### Piping & Redirecting Standard Output

The pipe (`|`) and redirect (`>`) operators are used for manipulating the standard output.

```ion
command arg1 | other_command | another_command arg2
command arg1 > file
```

### Piping & Redirecting Standard Error

The `^|` and `^>` operators are used for manipulating the standard error.

```ion
command arg1 ^| other_command
command arg1 ^> file
```

### Piping & Redirecting Both Standard Output & Standard Error

The `&|` and `&>` operators are used for manipulating both the standard output and error.

```ion
command arg1 &| other_command # Not supported yet
command arg1 &> file
```

### Conditional Operators

The Ion shell supports the `&&` and `||` operators in the same manner as the Bash shell. The `&&` operator
executes the following command if the previous command exited with a successful exit status. The `||`
operator performs the reverse -- executing if the previous command exited in failure.

```ion
test -e .git && echo Git directory exists || echo Git directory does not exist
```

### If Conditions

It is also possible to perform more advanced conditional expressions using the `if`, `else if`, and `else` keywords.

```ion
let a = 5;
if test $a -lt 5
    echo "a < 5"
else if test $a -eq 5
    echo "a == 5"
else
    echo "a > 5"
end
```

### While Loops

While loops will evaluate a supplied expression for each iteration and execute all the contained statements if it
evaluates to a successful exit status.

```ion
let a = 1
while test $a -lt 100
    echo $a
    let a += 1
end
```

### For Loops

For loops, on the other hand, will take a variable followed by a list of values or a range expression, and
iterate through all contained statements until all values have been exhausted. If the variable is `_`, it
will be ignored. Take note that quoting rules are reversed for for loops, and values from string-based command
substitutions are split by lines.

```ion
# Obtaining Values From a Subshell
for a in $(seq 1 10)
    echo $a
end

# Values Provided Directly
for a in 1 2 3 4 5
    echo $a
end

# Exclusive Range
for a in 1..11
    echo $a
end

# Inclusive Range
for a in 1...10
    echo $a
end

# Ignore Value
for _ in 1..10
   do_something
end

# Brace Ranges
for a in {1..10}
    echo $a
end

# Globbing
for a in *
    echo $a
end
```

### Command Substitution

Command substitution allows the user to execute commands within a subshell, and have the data written to standard
output used as the substitution for the expansion. There are two methods of performing command substitution: string and
array-based command substitution. String-based command substitutions are the standard, and they are created by wrapping
the external command between **$(** and **)**. Array-based command substitution is denoted by wrapping the command
between **@[** and **]**. The first merely captures the result as a single string, precisely as it was written, while
the second splits the data recieved into words delimited by whitespaces.

Try comparing the following:

```ion
for i in $(echo 1 2 3)
    echo $i
end
```

```ion
for i in @[echo 1 2 3]
    echo $i
end
```

### Slicing String-Based Command Substitutions

You may slice the string returned to obtain its substring:

```ion
echo $(echo one two three)[..3]
```

### Slicing Array-Based Command Substitutions

You may slice the array returned to obtained a specific set of elements:

```ion
echo @[grep "model name" /proc/cpuinfo | head -1][3..5]
```

### Functions

Functions in the Ion shell are defined with a name along with a set of variables. The function
will check if the correct number of arguments were supplied and execute if all arguments
were given.

```ion
fn fib n
    if test $n -le 1
        echo $n
    else
        let output = 1
        let previous = 1
        for _ in 2..$n
            let temp = $output
            let output += $previous
            let previous = $temp
        end
        echo $output
    end
end

for i in 1..20
    fib $i
end
```


### Executing Scripts with Array Arguments

Arguments supplied to a script are stored in the `@args` array.

#### Command executed

```ion
script.ion one two three
```

#### Script Contents

```ion
for argument in @args
    echo $argument
end
```

#### Output

```
script.ion
one
two
three
```
