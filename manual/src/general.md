# General rules

## Performance: Let Arithmetic vs Arithmetic Expansions
**let** arithmetic is generally faster than **$(())** expansions. The arithmetic expansions
should be used for increasing readability, or more complex arithmetic. If speed is important:
Multiple *let arithmetic statements will tend to be faster* than a single arithmetic expansion.

## Quoting Rules
- Variables are expanded in double quotes, but not single quotes.
- Braces are expanded when unquoted, but not when quoted.

## XDG App Dirs Support
All files created by Ion can be found in their respective XDG application directories. For example,
the init file for Ion can be found in **$HOME/.config/ion/initrc** on Linux systems; and the
history file can be found at **$HOME/.local/share/ion/history**. On the first launch of Ion, a
message will be given to indicate the location of these files.
