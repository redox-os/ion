# Ion Shell Parser Logic

This module handles all of the parsing logic within the Ion shell. The following is the strategy currently in use:

1. Parse supplied commands into individual statements using the `StatementSplitter`.
2. Print error if statement contains a syntax error, otherwise pass valid statement on.
3. Map each individual statement to their equivalent `Statement` enum using the peg parser.
4. Later expand shell expressions where required using the `expand_string()` function.

## Parsing Statements

First, inputs received by the shell should be parsed with the `StatementSplitter` in the `statements` module. A statement is any command that is separated by a `;`. Syntax errors are also detected in the `StatementSplitter`, where if an error is found, that statement will be skipped,
and an error message will be printed.

Given the following command:

```ion
let a = 1; while test $a -lt 100; echo $a; let a += 1; end
```

The `StatementSplitter` will parse the string and split it into individual statements. This makes the parsing that comes after much easier to manage. Example below, with one statement per line:

```ion
let a = 1
while test $a -lt 100
    echo $a
    let a += 1
end
```

### PEG Parser

Currently, PEG is being used to perform some basic parsing of syntax, but it has a limitation in that it cannot return string references, so at some point it may be replaced for a better solution that can avoid the needless copies.

The PEG parser will read a supplied statement and determine what kind of statement the Statement is -- collecting the required information for that statement and serving it back up as a `Statement` enum. This will later be pattern matched in the actual shell code to determine which code to execute.

#### Pipelines Module

The `pipelines` module is closely related to our `peg` module, in that for a handful of scenarios, such as when parsing `while`, `if`, and regular statements, the `pipelines` module provides a parser that parses pipelines, redirections, and conditional operators in commands, such as the following:

##### Pipelines Example

```ion
git remote show local | egrep 'tracked|new' | grep -v master | awk '{print $1}'
```

##### Conditionals Example

```ion
test -e .git && echo $PWD contains .git directory || echo $PWD does not contain a .git directory
```

##### Redirection Example

```ion
cargo build > build.log
```

#### Loops Module

For loops within Ion work uniquely compared to other shells, in that not only does the `ForExpression` parser parse/expand the supplied expression, but it checks if the expanded expression is either an inclusive or exclusive range, then returns the appropriate `ForExpression`.

### Shell Expansion

The purpose of the `shell_expand` module is to supply a generic expansion library that performs all shell expansions
throughout the shell. It reduces statements into a set of primitive words, and applies specific expansion rules
accordingly.
