- Feature Name: variable-expansions
- Start Date: 2018-06-14
- RFC PR: N/A
- Ion Issue: N/A

# Summary
[summary]: #summary

Ion allows variable expansion through invoking a variable with either the `$` or `@` sigils. The
sigil determines the type that should be returned from a variable expansion. As with other shells,
there are restrictions to the characters allowed in a variable name to make it easier to mix
variable and text together. It is also possible to explicitly declare the name of a variable by
enclosing it within braces (`{}`). Variables may also be expanded in unique ways, based on how the
expansion is constructed.

# Motivation
[motivation]: #motivation

POSIX shells have confusing syntax for manipulating strings and arrays. Having a clear distinction
makes it possible to go beyond what POSIX shells are capable of doing. String slicing, and string
methods, for example. The `$` and `@` sigils have prior precedence in previous languages, so the
decision was made to keep these for the same purpose.

# Detailed design
[design]: #detailed-design

Variables may exist anywhere within text. To make it easier for humans and the parser to read
these variables, it's important to be able to know where in the text that a variable's name ends,
as well as where it begins. The `$` and `@` sigils are used to denote where a variable's name
begins.

## Character Restrictions
[names]: #names

Characters from a to z, A to Z, 0 to 9, and _ are allowed for the construction of variable names.
If the shell encounters any character outside of this range, the shell will attempt to expand the
name that it found.

## Parsing Rules
[parsing-rules]: #parsing-rules

If the character that follows the sigil is `{`, then the shell will read all characters that
follow, until it encounters the corresponding `}`, at which point the name will be checked for
invalid characters. This is called a braced variable expansion.

```
$ echo ${foo}${bar}
```

If the character that follows is a valid character, then each character that follows will be
read until an invalid character is found, and the array of valid characters used as the variable's
name to expand. This is a standard variable expansion.

```
$ let foo bar method = ion_ shell exec
$ echo $foo$bar.$method()
ion_shell.exec()
```

If the character that follows is an invalid character, then no attempt to expand should be found,
and the sigil kept in place as being no different than any other standard character in the text.

> Note that there are some special variables that are accessible from $ that contain invalid
characters. `$?`, for example, gets the last command's exit status.

## String Expansion Rules
[string-expansion]: #string-expansion

### Quoting
[quoted-string]: #quoted-string

When a string expansion is not quoted, then newlines will be replaced with spaces. This is a
design decision which stems from prior POSIX shells, which can make it easier to handle data
which should not treat newlines differently from spaces. In order to preserve newlines, the
expansion must be enclosed within double quotes.

```
$ let foo = "FOO
BAR"
$ echo $foo
FOO BAR
$ echo "$foo"
FOO
BAR
```

### String Slicing
[string-slicing]: #string-slicing

A big benefit of making a distinction between strings and arrays is that we are able to
efficiently perform string slicing without needing to rely on external tooling to do the task
for us. Any string expansion may index into a string by directly following the expansion with
`[value]`. The `value` may either be an index, or a range.

> String slicing is based on graphemes

```
$ let foo = BAR
$ echo $FOO[1]
A
$ echo ${FOO}[1]
A
$ echo $FOO[..2]
BA
```

## Array Expansion Rules
[array-expansion]: #array-expansion

### Quoting
[quoted-array]: #quoted-array

When an array is double-quoted, the array's contents will be joined into a string, with
a space between each array element. There are times when an array will still be coerced
into a string, however, such as when assigning a value with `let` using an array which is not
wrapped within brackets.

```
$ for value in @array; echo $value; end
1
2
3
4
$ for value in "@array"; echo $value; end
1 2 3 4
```

### Array Slicing
[array-slicing]: #array-slicing

The same slicing rules that applies for strings also apply here, but rather than slicing on
graphemes within a string, the result is slicing elements from an array. Slicing by index
will return a string, whereas slicing by range will return a new array.

```
$ let a = [ 1 2 3 4 ]
$ echo @array[..3]
1 2 3
$ echo @array[2]
3
```

# Drawbacks
[drawbacks]: #drawbacks

There are no known drawbacks.

# Alternatives
[alternatives]: #alternatives

POSIX combines arrays with strings. This is not flexible.

# Unresolved questions
[unresolved]: #unresolved-questions

There are no unresolved questions.
