## Scopes
[scopes]: #scopes

Ion, like most other languages, has a concept of scopes. Any variables defined should belong to
the same "body" they are defined in. For example, a variable defined in an if-statement should not
be visible outside of it. Definitions in ion are done using `let`, which can also be used for
shadowing a variable. Updating an existing variable is done using `assign`. Therefore, the
technical difference is that `assign` works with scopes and fails if the variable doesn't exist.

```ion
let x = 5 # defines x
let y = 3 # defines y

# This will always execute.
# Only reason for this check is to show how
# variables defined inside it are destroyed.
if test 1 == 1
  assign x = 4 # updates existing x
  let y = 2 # defines (shadows) y

  # end of scope, y is deleted since it's owned by it
end

echo $x # prints 4
echo $y # prints 3
```

## Scopes and functions
[scopes-and-functions]: #scopes-and-functions

Functions can use functions and variables from higher-level scopes than the one they were defined
in. They can not access data from the scope they were called from, nor can they access variables
defined after they were defined, meaning ion would keep some sort of ordering of what is defined
when. Other functions would be excluded from this order as it's important for functions to be able
to call each other. Once again, this matches the behavior of most other languages.
