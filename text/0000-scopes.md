- Feature Name: scopes
- Start Date: (fill me in with today's date, YYYY-MM-DD)
- RFC PR: (leave this empty)
- Ion Issue: (leave this empty)

# Summary
[summary]: #summary

One para explanation of the feature.

# Motivation
[motivation]: #motivation

Ion intends to be a next-generation shell with some high level features that were previously
reserved to programming languages. Scopes are an essential component in programming langauges
that can increase the flexibility and reliability of variable assignment and recalling, as
well as enabling better resource management once scopes are no longer in scope.

It is also vital to have them four our specification for functions, which should only have
access to the variables that were used as input arguments.

# Detailed design
[design]: #detailed-design

## Scopes
[scopes]: #scopes

Variables and functions within Ion adhere to the concept of scopes, which exists in many other
programming languages. A scope is a block of statements that are executed together within the
same body, such as a branch within an `if` statement, the body of a `for` or `while` loop, or
the body of a function. Scopes may even be nested within other scopes.

TODO: Add image here

Variables may be accessed from scopes that exist on a higher level than the scope currently in
execution. However, variables created at lower scopes will be lost once that scope has exited.
For example, a variable defined in an if-statement should not be visible outside of it.

TODO: Add image here

Variables are created within a scope using the `let` keyword. If the `let` keyword is used to
create a variable which already exists in a higher scope, then the new variable will be assigned
in the current scope, **shadowing** the variable which already exists. Shadowing is a term which
means that the original variable will exist, but the new variable will override it for as long
as it exists.

TODO: Add image here

To update an existing variable in a higher scope, rather than shadowing it, the `assign` keyword
should be used instead. This keyword cannot create new variables, but it may update them.

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

## Scopes and Functions
[scopes-and-functions]: #scopes-and-functions

Functions may not access variables that are outside of the function by default. In order to access
variables that were declared outside of the function, the variable must be invoked through either
the `global` or `super` namespace.

### Super Namespace
[super-namespace]: #super-namespace

The `super` namespace allows the shell access a variable directly outside of where the function was
defined. This namespace may be repeated any amount of times to access a variable higher up in the
scope stack. For example, `${super::super::a}` accesses `$a` from two scopes up.

```
TODO EXAMPLE
```

### Global Namespace
[global-namespace]: #global-namespace

The `global` namespace accesses variables from any higher scope. It is equivalent to repeating the
`super` namespace X amount of times, where X is the number of nested scopes required to reach that
variable.

```
TODO EXAMPLE
```

### Restriction
[function-scope-restrictions]: #function-scope-restrictions

Variables defined after the function should never be accessible, meaning there needs to
be some sort of ordering to insertions.  Note that together these restrictions make sure it doesn't
matter where the function is called from, only where it's defined.


# Drawbacks
[drawbacks]: #drawbacks

Why should we *not* do this?

# Alternatives
[alternatives]: #alternatives

What other designs have been considered? What is the impact of not doing this?

# Unresolved questions
[unresolved]: #unresolved-questions

What parts of the design are still TBD?
