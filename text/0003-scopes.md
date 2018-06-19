## Scopes
[scopes]: #scopes

Ion, like most other languages, has a concept of scopes. Any variables defined should belong to the
same "body" they are defined in. For example, a variable defined in an if-statement should not be
visible outside of it. Definitions in ion are done using `let`, which can also be used for shadowing
a variable. Updating an existing variable is done using `assign`. Therefore, the technical
difference is that `assign` works with scopes and fails if the variable doesn't exist.

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

Functions cannot access any variables (note, functions can still be accessed) from outside of it,
unless you specify "namespace". The `super::` namespace lets you access variables directly outside
of where the function was defined.  This namespace may be repeated any amount of times to access a
variable higher up. For example, `super::super::a` accesses `a` from two nested functions up. The
`global::` namespace accesses the top-level scopes before the first nested function, and is
technically just `super::` automatically repeated X amount of times, where X is the number of nested
functions.  Variables defined after the function should never be accessible, meaning there needs to
be some sort of ordering to insertions.  Note that together these restrictions make sure it doesn't
matter where the function is called from, only where it's defined.
