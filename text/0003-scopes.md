## Scopes
[scopes]: #scopes

Ion, like most other languages, has a concept of scopes. Any variables defined should belong to
the same "body" they are defined in. For example, a variable defined in an if-statement should not
be visible outside of it. Because ion does not have a separate syntax for defining/updating a
variable it assumes first assignment is the declaration.

## Scopes and functions
[scopes-and-functions]: #scopes-and-functions

Functions will be called from the scope they were defined in, meaning definitions in another scope
won't be visible from within the function, even if the function is called from within said scope.
Currently ion does however allow you to access a variable defined between the function and the
function call from within said function, as long as it is not in a new scope. This does not match
most other languages, but is similar to the way python does it. This may be subject to change as
it's due to how variables are implemented rather than a design decision. More specifically, ion
keeps a hashmap of variables, and hashmaps do not have ordering.
