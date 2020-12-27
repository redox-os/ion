# Variables
The `let` builtin is used to create local variables within the shell, and apply basic arithmetic
to variables. The `export` keyword may be used to do the same for the creation of external
variables. Variables cannot be created the POSIX way, as the POSIX way is awkard to read/write
and parse.
```sh
{{#include ../../../tests/variables.ion:variables}}
```
```txt
{{#include ../../../tests/variables.out:6:7}}
```

## Multiple Assignments
Ion also supports setting multiple values at the same time
```sh
{{#include ../../../tests/variables.ion:multiple_assignment}}
```
```txt
{{#include ../../../tests/variables.out:9:12}}
```

## Type-Checked Assignments
It's also possible to designate the type that a variable is allowed to be initialized with.
Boolean type assignments will also normalize inputs into either `true` or `false`. When an
invalid value is supplied, the assignment operation will fail and an error message will be
printed. All assignments after the failed assignment will be ignored.
```sh
{{#include ../../../tests/variables.ion:type_checked_assignment}}
```
```txt
{{#include ../../../tests/variables.out:14:19}}
```

## Dropping Variables

Variables may be dropped from a scope with the `drop` keyword. Considering that a variable
can only be assigned to one type at a time, this will drop whichever value is assigned to
that type.
```sh
{{#include ../../../tests/variables.ion:dropping_variables}}
```

## Supported Primitive Types

- `str`: A string, the essential primitive of a shell.
- `bool`: A value which is either `true` or `false`.
- `int`: An integer is any whole number.
- `float`: A float is a rational number (fractions represented as a decimal).

## Arrays

The `[T]` type, where `T` is a primitive, is an array of that primitive type.

## Maps

Likewise, `hmap[T]` and `bmap[T]` work in a similar fashion, but are a collection
of key/value pairs, where the key is always a `str`, and the value is defined by the
`T`.
