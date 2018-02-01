# Features

## Miscellaneous Features

Small features that don't belong in any specific category.

[Miscellaneous Features](ch03-00-miscellaneous.html)


## Shell Expansions / Substitutions

Expansions provide dynamic string generation capabilities. These work identical to the standard
POSIX way, but there are a few major differences: arrays are denoted with an **@** sigil, and have
their own variant of process expansions (**@()**) which splits outputs by whitespace; the
arithmetic logic is more feature-complete, supports floating-point math, and handles larger
numbers; and Ion supports methods in the same manner as the [Oil shell](http://www.oilshell.org/).

[Expansions](ch05-00-expansions.html)
- [Variable Expansions](ch05-01-variable.html)
- [Process Expansions](ch05-02-process.html)
- [Brace Expansions](ch05-03-brace.html)
- [Arithmetic Expansions](ch05-04-arithmetic.html)
- [Method Expansions](ch05-05-method.html)

## Slicing Syntax

A critical feature over POSIX shells, Ion provides the ability to slice expansions with a familiar
syntax. The supplied index or range is expanded, and then handled according to whether the
expanded value is an index, inclusive range, or exclusive range. This eliminates much of the need
for temporarily storing and/or piping values to other commands, instead performing the slicing at
parse-time.

[Slicing Syntax](ch06-00-slicing.html)

## Control Flow

As Ion features an imperative paradigm, the order that statements are evaluated and executed is
determined by various control flow keywords, such as `if`, `while`, `for`, `break`, and
`continue`. Ion's control flow logic is very similar to POSIX shells, but there are a few major
differences, such as that all blocks are ended with the `end` keyword; and the `do`/`then`
keywords aren't necessary.

[Control Flow](ch07-00-flow.html)
- [Conditionals](ch07-01-conditionals.html)
- [Loops](ch07-02-loops.html)
- [Matches](ch07-03-matches.html)
