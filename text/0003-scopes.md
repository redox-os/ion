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

Functions will be called from the scope they were defined in, meaning definitions in another scope
won't be visible from within the function, even if the function is called from within said scope.
They will also keep track of in which order they were defined so they cannot access values defined
after the function itself. Once again, this matches the behavior of most other languages.
