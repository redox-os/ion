# Migrating from POSIX Shells

## Notable changes
 - Arrays are full-class citizens, using the @ sigil. That means emails and git urls must be single quoted
 - The shell has proper scopes (variables get unset after the end of the definition scope), and functions are closures
 - The shell has an internal variable store. That means environment variables must be explicitely exported to be available to commands.
 - For now, per-command environment variables are not supported (ex: `LANG=it_CH.utf8 man man`)
 - The testing builtin (`[[ .. ]]`) was replaced with `test`, `exists`, and/or other commands
 - The control flow have been revisited, see the relevant part of the manual

## Customizing your prompt
 - Define the PROMPT function to be called whenever the prompt needs to be drawn. Simply print the prompt to stdout in the function (printf or git branch directly)
 - Variables are defined with all the colors (see the namespaces manual page for all details). This means you don't have to deal with all the escape codes directly. No more `\x1B[33m`, instead it's `${color::yellow}`.

## Customizing the autocompletion
 - Define the $SUGGESTION\_PROMPT variable to define a custom color/style for suggestions. The style is reset after the suggestion.
