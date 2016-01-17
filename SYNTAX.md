# ion Syntax Reference

## Current Syntax

### Commands
- `arg0 arg1 "arg 2"` will call the command `arg0` with three arguments, the executable path, `arg1`, and `arg 2`

### Builtins
- `help` will list all builtins
- `help builtin` will display the syntax and description of `builtin`

### Variables
- `variable=some value` will set a variable to `some value`, if a `some value` is blank the variable will be deleted
- `$variable` will be placed inline as a single argument, so `touch $variable` would try to create a file `some value`
- `$` will list all variables

### Conditionals
- `if left comparison right` will begin a comparison block
 - `left` and `right` are single arguments, they may be a variable like `$variable` or a value like `2` or `"some value"`
 - The available comparisons are `==`, `!=`, `>`, `>=`, `<`, and `<=`
- `else` will invert the comparison block
- `fi` will end the comparison block

## Proposed Syntax
