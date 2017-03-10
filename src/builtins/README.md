# Ion Shell Builtins

This directory contains the source code of Ion's builtin commands and documentation for their usage.

## Variables

The **variables.rs** module contains commands relating to setting and removing aliases, variables, and exports. The shell stores aliases and variables within two separate `BTreeMap` structures inside the same `Variables` structure, which is contained within the `Shell` structure.

### Alias

The `alias` command is used to set an alias for running other commands under a different name. The most common usages of the `alias` keyword are to shorten the keystrokes required to run a command and it's specific arguments, and to rename a command to something more familiar.

```ion
alias ls = 'ls --color'
```

If the command is executed without any arguments, it will simply list all available aliases.

The `unalias` command performs the reverse of `alias` in that it drops the value from existence.

```ion
unalias ls
```

### Let

The `let` command sets a variable to the value of the expression that follows. These variables are stored as local values within the shell, so other processes many not access these values.

```ion
// TODO: Ion Shell does not yet implement stderr redirection.
let git_branch = $(git rev-parse --abbrev-ref HEAD 2> /dev/null)
```

If the command is executed without any arguments, it will simply list all available variables.

#### Dropping variables

To drop a value from the shell, the `drop` keyword may be used:

```ion
drop git_branch
```

#### Arithmetic

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

The `export` command works similarly to the `let` command, but instead of defining a local variable, it defines a global variable that other processes can access.

```sh
export PATH = "~/.cargo/bin:${PATH}"
```

#### Arithmetic

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
