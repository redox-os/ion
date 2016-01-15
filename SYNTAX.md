# ion Syntax Reference

## Current Syntax

### Commands
- `arg0 arg1 "arg 2"` will call the command `arg0` with two arguments, `arg1` and `arg 2`

### Builtins
- `help` will list all builtins
- `help builtin` will list the syntax and description of that builtin

### Variables
- `variable=some value` will set a variable to `some value`
- `$variable` will be placed inline as a single argument, so `touch $variable` would try to create a file `some value`
- `$` will show all variables
- `variable=` will remove a variable

### Conditionals
- `if left comparison right` will begin a comparison block
 - `left` and `right` are single arguments, they may be a variable like `$variable` or a value like `2` or `"some value"`
 - The available comparisons are `==`, `!=`, `>`, `>=`, `<`, and `<=`
- `else` will invert the comparison block
- `fi` will end the comparison block

## Proposed Syntax
