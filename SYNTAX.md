# ion Syntax Reference

## Current Syntax

### Commands
- `arg0 arg1 "arg 2"` will call the command `arg0` with three arguments, the executable path, `arg1`, and `arg 2`

### Builtins
- `help` will list all builtins
- `help builtin` will display the syntax and description of `builtin`

### Variables
- `variable=some value` will set a variable to `some value`, if `some value` is blank the variable will be deleted
- `$variable` will be placed inline as a single argument, so `touch $variable` would try to create a file `some value`
- `$` will list all variables

### Conditionals
- `if left comparison right` will begin a comparison block
 - `left` and `right` are single arguments, they may be a variable like `$variable` or a value like `2` or `"some value"`
 - The available comparisons are `==`, `!=`, `>`, `>=`, `<`, and `<=`
- `else` will invert the comparison block
- `fi` will end the comparison block

## Proposed Syntax

A LR(k) grammar. Something like this (pseudocode):
```
statement:
    LET IDENT SET expr
    | IF expr DELSTART statement* DELEND
expr:
    # comparations
    expr EQ expr = eq
    | expr NEQ expr = not_eq
    | expr LT expr = less_than
    | expr GT expr = greater_than
    | expr LEQ expr = less_than_or_eq
    | expr GEQ expr = greater_than_or_eq
    # operators
    | expr PLUS expr = add
    | expr MINUS expr = sub
    | expr MUL expr = mul
    | expr DIV expr = div
    # control
    | IF expr DELSTART expr DELEND ELSE DELSTART expr DELEND = if_else
    # misc
    | statement* SEMICOLON expr = block
    # const
    | STR(a) = str
    | NUM(a) = num
    ...
```

example:

```
let home = "~";

if home == pwd {
    echo "home sweet home"
}

// let's define a new cmd

fn my_cmd a b {
    echo a + b;
}

mycmd 2 4 // 6
```

