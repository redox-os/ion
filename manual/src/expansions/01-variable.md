# Variable Expansions

Expansions provide dynamic string generation capabilities. These work identical to the standard
POSIX way, but there are a few major differences: arrays are denoted with an **@** sigil, and have
their own variant of process expansions (**@()**) which splits outputs by whitespace; and our
arithmetic logic is destined to be both more feature-complete, supports floating-point math, and
handles larger numbers.

## String Variables

Like POSIX shells, the **$** sigil denotes that the following expression will be a string
expansion. If the character that follows is an accepted Unicode character, all characters that
follow will be collected until either a non-accepted Unicode character is found, or all characters
have been read. Then the characters that were collected will be used as the name of the string
variable to substitute with.

```sh
$ let string = "example string"
$ echo $string
> example string
$ echo $string:$string
> example string:example string
```

**NOTE:**
- Accepted characters are **unicode** alphanumeric characters and **_**.
- If not double quoted, newlines will be replaced with spaces.

## Array Variables

Unlike POSIX, Ion also offers support for first class arrays, which are denoted with the **@**
sigil. The rules for these are identical, but instead of returning a single string, it will
return an array of strings. This means that it's possible to use an array variable as arguments
in a command, as each element in the array will be treated as a separate shell word.

```sh
$ let array = [one two three]
$ echo @array
> one two three
$ cmd @args
```

However, do note that double-quoted arrays are coerced into strings, with spaces separating each
element. It is equivalent to using the `$join(array)` method. Containing multiple arrays within
double quotes is therefore equivalent to folding the elements into a single string.

## Braced Variables

Braces can also be used when you need to integrate a variable expansion along accepted Unicode
characters.

```sh
echo ${hello}world
echo @{hello}world
```

## Aliases

Ion also supports aliasing commands, which can be defined using the `alias` builtin. Aliases
are often used as shortcuts to repetitive command invocations.

```sh
alias ls = "ls --color"
```
