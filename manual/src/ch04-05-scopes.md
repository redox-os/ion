# Scopes

A scope is a batch of commands, often ended by `end`.
Things like `if`, `while`, etc all take a scope to execute.

In ion, just like most other languages, all variables are destroyed once the scope they were defined in is gone.
Similarly, variables from other scopes can still be overriden.
However, ion has no dedicated keyword for updating an existing variable currently,
so the first invokation of `let` gets to "own" the variable.

*This is an early implementation and will be improved upon with time*

```ion
let x = 5 # defines x

# This will always execute.
# Only reason for this check is to show how
# variables defined inside it are destroyed.
if test 1 == 1
  let x = 2 # updates existing x
  let y = 3 # defines y

  # end of scope, y is deleted since it's owned by it
end

echo $x # prints 2
echo $y # prints nothing, y is deleted already
```

## Functions

Functions have the scope they were defined in.
This ensures they don't use any unintended local variables that only work in some cases.
Once again, this matches the behavior of most other languages, apart from perhaps LOLCODE.

```ion
let x = 5 # defines x

fn print_vars
  echo $x # prints 2 because it was updated before the function was called
  echo $y # prints nothing, y is owned by another scope
end

if test 1 == 1
  let x = 2 # updates existing x
  let y = 3 # defines y
  print_vars
end
```
