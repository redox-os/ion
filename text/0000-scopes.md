- Feature Name: scopes
- Start Date: (fill me in with today's date, YYYY-MM-DD)
- RFC PR: (leave this empty)
- Ion Issue: (leave this empty)

# Summary
[summary]: #summary

In order to prevent unexpected behavior when defining and calling functions,
they should have a separate set of variables. Each "body" (the part that is
usually indented) will own its own map of variables, its scope. Each variable
owned by it is deleted when the body ends. To access a variable in a different
scope you can use `assign` to make the intention of modifying the existing
variable obvious. Functions disallow you from accessing outer variables by
default, but this can be overriden with namespaces to specify where you want
the variable from.

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

![example image](https://gitlab.redox-os.org/redox-os/ion/raw/rfcs/images/blocks.svg)

Variables may be accessed from scopes that exist on a higher level than the scope currently in
execution. However, variables created at lower scopes will be lost once that scope has exited.
For example, a variable defined in an if-statement should not be visible outside of it.

![example image](https://gitlab.redox-os.org/redox-os/ion/raw/rfcs/images/scopes.svg)

Variables are created within a scope using the `let` keyword. If the `let` keyword is used to
create a variable which already exists in a higher scope, then the new variable will be assigned
in the current scope, **shadowing** the variable which already exists. Shadowing is a term which
means that the original variable will exist, but the new variable will override it for as long
as it exists.

![example image](https://gitlab.redox-os.org/redox-os/ion/raw/rfcs/images/scopes.svg)

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
fn parent
  let a = 2

  fn child
    let a = 4        # shadows a
    echo $a          # prints 4
    echo ${super::a} # prints 2
  end

  child
end

parent
```

### Global Namespace
[global-namespace]: #global-namespace

The `global` namespace accesses variables from any higher scope. It is equivalent to repeating the
`super` namespace X amount of times, where X is the number of nested scopes required to reach that
variable.

```
let a = 1

fn parent
  fn child
    echo ${global::a} # prints 1
  end
end

parent
```

Global variales sometimes need to be mutated to maintain a global state. You
can do this using the `--global` flag to `assign`.

```
let greeted = 0

fn greet_once
  if test greeted == 0
    echo "Hello!"
    assign --global greeted = 1
  end
end
```

### Restriction
[function-scope-restrictions]: #function-scope-restrictions

Variables defined after the function should never be accessible, meaning there needs to
be some sort of ordering to insertions. Note that together these restrictions make sure it doesn't
matter where the function is called from, only where it's defined.

# Drawbacks
[drawbacks]: #drawbacks

Functions can no longer mutate outer environment, which might make things more
complicated. It also adds complexity to the language and code, and shells
usually have very simple syntax. However, this would improve the overall
quality of scripts written in ion so it's probably worth it anyway. Namespaces,
ordering and even `assign` are features we could live without, but a basic
scoping mechanism would improve things a lot.

# Alternatives
[alternatives]: #alternatives

We could let local variables have their own `local` keyword, which wouldn't be
the default. However, then everybody would just be pushed to use that and the
old `let` syntax would be wasted. Alternatively we could keep this RFC but also
add a `global` keyword for bypassing scopes. However global variables are
usually harder to find that way rather than letting each global variable be
defined in the global scope where you can easily see all of them.

# Unresolved questions
[unresolved]: #unresolved-questions

How in the world are you supposed to mutate variables outside of a function???
Ion doesn't have references yet so right now return values are the only option.
