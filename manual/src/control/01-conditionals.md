# Conditionals

Conditionals in a language are a means of describing blocks of code that may potentially execute,
so long as certain conditions are met. In Ion, as with every other language, we support this
via **if statements**, but unlike POSIX shells, we have a cleaner syntax that will require less
boilerplate, and increase readability.

## If Statements

The `if` keyword in Ion is designated as a control flow keyword, which works similar to a builtin
command. The `if` builtin will have it's supplied expression parsed and executed. The return
status of the executed command will then be utilized as a boolean value. Due to the nature
of how shells operate though, a logical `true` result is a `0` exit status, which is an exit
status that commands return when no errors are reported. If the value is not zero, it is
considered `false`. Sadly, we can't go back in time to tell early UNIX application developers
that `1` should indicate success, and `0` should indicate a general error, so this behavior
found in POSIX shells will be preserved.

We supply a number of builtin commands that are utilized for the purpose of evaluating
expressions and values that we create within our shell. One of these commands is the [`test`
builtin](../builtins.md#test---perform-tests-on-files-and-text), which is commonly found
in other POSIX shells, and whose flags and operation should be identical.
We also supply a `not` builtin, which may be convenient to use in conjuction with other commands
in order to flip the exit status; and a `matches` builtin that performs a regex-based boolean match.

```sh
if test "foo" = $foo
    echo "Found foo"
else if matches $foo '[A-Ma-m]\w+'
    echo "we found a word that starts with A-M"
    if not matches $foo '[A]'
        echo "The word doesn't start with A"
    else
        echo "The word starts with 'A'"
    end
else
    echo "Incompatible word found"
end
```

A major distinction with POSIX shells is that Ion does not require that the if
statement is followed with a `then` keyword. The `else if` statements are also written
as two separate words, rather than as `elif` which is POSIX. And all blocks in Ion are ended
with the `end` keyword, rather than `fi` to end an if statement. There is absolutely zero logical
reason for a shell language to have multiple different keywords to end different expressions.

## Complete List of Conditional Builtins

- [x] and
- [x] contains
- [x] exists
- [x] eq
- [ ] intersects
- [x] is
- [x] isatty
- [x] matches
- [x] not
- [x] or
- [x] test
- [ ] < (Polish Notation)
- [ ] <= (Polish Notation)
- [ ] &gt; (Polish Notation)
- [ ] &gt;= (Polish Notation)
- [ ] = (Polish Notation)

## Using the **&&** and **||** Operators

We also support performing conditional execution that can be performed within job execution,
using the same familiar POSIX syntax. The **&&** operator denotes that the following command
should only be executed if the previous command had a successful return status. The **||**
operator is therefore the exact opposite. These can be chained together so that jobs
can be skipped over and conditionally-executed based on return status. This enables succintly
expressing some patterns better than could be done with an if statement.

```sh
if test $foo = "foo" && test $bar = "bar"
    echo "foobar was found"
else
    echo "either foo or bar was not found"
end
```

```sh
test $foo = "foo" && test $bar = "bar" &&
    echo "foobar was found" ||
    echo "either foo or bar was not found"
```
