# Features

Below is an overview of features that Ion has either already implemented, or aims to implement in
the future. If you have any ideas for features that Ion hasn't considered, you are welcome to
open an issue on the project's GitHub.

- Misc
  - [x] Implicit `cd`
  - [x] XDG App Dirs
- Variables
  - [x] String Variables
  - [x] Array Variables
  - [x] Aliases
  - [ ] Associative Arrays
- Shell Expansions
  - Variable Expansions
    - [x] String Expansions (**$string**, **${string}**)
    - [x] Array Expansions (**@array**, **@{array}**)
  - Process Expansions
    - [x] String Process Expansions (**$(command args...)**)
    - [x] Array Process Expansions (**@(command args...)**)
  - Brace Expansions (**abc{1,2,3}def{1,2,3}ghi**)
    - [x] Brace Ranges
    - [x] Nested Braces
    - [x] Permutated Braces
  - [x] Arithmetic Expansions (**$((5 * 10 / 3.5))**)
  - Method Expansions
    - [x] String Methods (**$method(args...)**)
    - [x] Array Methods (**@method(args...)**)
    - [x] Inline Methods (Expressions Within Methods)
- Slicing Syntax
  - [x] String Variable Slicing (**$string[5..10]**)
  - [x] Array Variable Slicing (**@array[5..10]**)
  - [x] Array Slicing (**[one two three][2]**)
  - [x] String Process Slicing (**$(command args...)[15..]**)
  - [x] Array Process Slicing (**@(command args...)[1]**)
- Flow Control
  - [x] For Loops
  - [x] While Loops
  - [x] If Statements
  - [x] Match Statements (Incomplete)
  - [ ] Match All Statements
  - [ ] For Match Loop
  - [ ] Foreach Loops
- Functions
  - [x] Optionally-Typed Function Parameters
  - [x] Descriptions
  - [ ] Local Scopes & Dynamic Variables
  - [ ] Piping / Redirecting Functions
  - [ ] Backgrounding Functions
  - [ ] Execute Function When Variable Changes
- Builtin Commands
  - [ ] Piping / Redirecting Builtins
  - [ ] Implemented All Builtins
  - [ ] Completed Help Documentation
- [x] Script Executions
- [x] Signal Handling
- Job Control
  - [x] `bg`, `fg`, `jobs`, etc.
  - [x] Send jobs to background with **&**
- Plugins Support
  - [ ] Builtin Plugins
  - [ ] Prompt Plugins
  - [ ] Syntax Plugins


## Miscellanious Features

Small features that don't belong in any specific category.

[Miscellanious Features](./miscellanious.md)


## Shell Expansions / Substitutions

Expansions provide dynamic string generation capabilities. These work identical to the standard
POSIX way, but there are a few major differences: arrays are denoted with an **@** sigil, and have
their own variant of process expansions (**@()**) which splits outputs by whitespace; the
arithmetic logic is more feature-complete, supports floating-point math, and handles larger
numbers; and Ion supports methods in the same manner as the [Oil shell](http://www.oilshell.org/).

- [Variable Expansions](expansions/variable.md)
- [Process Expansions](expansions/process.md)
- [Brace Expansions](expansions/brace.md)
- [Arithmetic Expansions](expansions/arithmetic.md)
- [Method Expansions](expansions/methods.md)

## Slicing Syntax

A critical feature over POSIX shells, Ion provides the ability to slice expansions with a familiar
syntax. The supplied index or range is expanded, and then handled according to whether the
expanded value is an index, inclusive range, or exclusive range. This eliminates much of the need
for temporarily storing and/or piping values to other commands, instead performing the slicing at
parse-time.

## Flow Control
