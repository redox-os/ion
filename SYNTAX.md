# ion Syntax Reference

## Current Syntax

### Commands
- `arg0 arg1 "arg 2"` will call the command `arg0` with three arguments, the executable path, `arg1`, and `arg 2`

### Builtins
- `help` will list all builtins
- `help builtin` will display the syntax and description of the `builtin` command

### Variables
- `let variable=value` will set a variable to `value`
- `$variable` will be placed inline as a single argument, so `touch $variable` would try to create a file `some value`
- `drop variable` will delete the variable called `variable`
- `let` will list all variables

### Conditionals
- `if left comparison right` will begin a comparison block
 - `left` and `right` are single arguments, they may be a variable like `$variable` or a value like `2` or `"some value"`
 - The available comparisons are `==`, `!=`, `>`, `>=`, `<`, and `<=`
- `else` will invert the comparison block
- `end` will end the comparison block

### Functions
Use the `fn` keyword to define functions:
```
fn function
  echo "Hello, Ion!"
end
```
And use the function name to call it:
```
ion:# function
Hello, Ion!
```
You can also create function with arguments:
```
fn print_two_strings first second
  echo $first $second
end
```
To call the function you can use the function name followed by the arguments:
```
ion:# print_two_strings "Foo" "Bar"
Foo Bar
```

### Piping
- `echo foo | cat | xargs touch` will pipe the output from one process to another.

### Redirection
- `echo foo > bar` will write "foo" to a file named "bar".
- `cat < foo` will write the contents of a file named "foo" to the console.
- `cat < foo > bar` will write the contents of a file named "foo" to a file named "bar".

## Proposed Syntax

A LR(k) grammar. This is a rough brainstorm and is somewhat out of sync with the examples:
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

Examples:

```
let home = "~";

if home == pwd
    echo "home sweet home"
end

// let's define a new cmd

fn my_cmd a b
    echo a + b
end

mycmd 2 4 // 6
```

