# Shell Expand Module

This module reduces statements into a set of word token primitives, and applies expansions to these tokens accordingly.
In order to be modular, the expansion logic is supplied with a structure containing closure references, which are
provided higher up by the shell.

## Tokenizing Words

The `words` module contains the `WordIterator` which reduces statments into set of `WordTokens`. The following are
supported word tokens:

- `Normal(&str)`: Normal words that do not require any expansion at all
- `Whitespace(&str)`: Whitespace between words
- `Variable(&str, bool)`: A variable name and an indication of if it was quoted
- `Process(&str, bool)`: A subshell and an indication of if it was quoted
- `Tilde(&str)`: A tilde expression such as ~ or ~user
- `Brace(Vec<&str>)`: A brace expansion and each of its inner elements
