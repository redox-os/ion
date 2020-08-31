# Variables

The `let` builtin is used to create local variables within the shell, and apply basic arithmetic
to variables. The `export` keyword may be used to do the same for the creation of external
variables. Variables cannot be created the POSIX way, as the POSIX way is awkard to read/write
and parse.

```sh
let string_variable = "hello string"
let array_variable = [ hello array ]
```

## Multiple Assignments

Ion also supports setting multiple values at the same time

```sh
let a b = one two
echo $a
echo $b

let a b = one [two three four]
echo $a
echo @b
```

#### Output

```
one
two
one
two three four
```

## Type-Checked Assignments

It's also possible to designate the type that a variable is allowed to be initialized with.
Boolean type assignments will also normalize inputs into either `true` or `false`. When an
invalid value is supplied, the assignment operation will fail and an error message will be
printed. All assignments after the failed assignment will be ignored.

```sh
let a:bool = 1
let b:bool = true
let c:bool = n
echo $a $b $c


let a:str b:[str] c:int d:[float] = one [two three] 4 [5.1 6.2 7.3]
echo $a
echo @b
echo $c
echo @d
```

#### Output

```
true
true
false
one
two three
4
5.1 6.2 7.3
```

## Dropping Variables

Variables may be dropped from a scope with the `drop` keyword. Considering that a variable
can only be assigned to one type at a time, this will drop whichever value is assigned to
that type.

```
let string = "hello"
drop string
let array = [ hello world ]
drop array
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
