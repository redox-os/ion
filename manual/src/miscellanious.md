# Miscellanious Features

These are features of Ion that don't belong to any specific category.

## Implicit `cd`

Like the [Friendly Interactive Shell](), Ion also supports executing the `cd` command automatically
when given a path. Paths are denoted by beginning with `.`/`/`/`~`, or ending with `/`.

```ion
~/Documents # cd ~/Documents
..          # cd ..
.config     # cd .config
examples/   # cd examples/
```

## XDG App Dirs Support

All files created by Ion can be found in their respective XDG application directories. In example,
the init file for Ion can be found in **$HOME/.config/ion/initrc** on Linux systems; and the
history file can be found at **$HOME/.local/share/ion/history**. On the first launch of Ion, a
message will be given to indicate the location of these files.

### Quoting Rules

In general, double quotes allow expansions within quoted text, whereas single quotes do not.
An exception to the rule is brace expansions, where double quotes are not allowed. When
arguments are parsed, the general rule is the replace newlines with spaces. When double-quoted
expansions will retain their newlines. Quoting rules are reversed for heredocs and for loops.

## Multi-line Arguments

If a line in your script becomes too long, you may signal to Ion to continue reading the next line
by appending an `\` character at the end of the line. This will ignore newlines.

```ion
command arg arg2 \
    arg3 arg4 \
    arg 5
```

## Multi-line Comments

If a comment needs to contain newlines, you may do so by having an open quote, as Ion will only
begin parsing supplied commands that are terminated. Either double or single quotes may be used
for this purpose, depending on which quoting rules that you need.

echo "This is the first line
this is the second line
this is the third line"


## General Tips

### Let Arithmetic vs Arithmetic Expansions

Using **let** arithmetic is generally faster than **$(())** expansions. The arithmetic expansions
should be used for increasing readability, or more complex arithmetic; but if speed is important,
multiple let arithmetic statements will tend to be faster than a single arithmetic expansion.
