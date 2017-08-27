# Let Arithmetic

Ion supports applying some basic arithmetic, one operation at a time, to string variables. To
specify to `let` to perform some arithmetic, designate the operation immediately before **=**.
Operators currently supported are:

- [x] Add (**+**)
- [x] Subtract (**-**)
- [x] Multiply (**\***)
- [x] Divide (**/**)
- [ ] Integer Divide (**//**)
- [ ] Modulus (**%**)
- [x] Powers  (**\*\***)

## Individual Assignments

The following examples are a demonstration of applying a mathematical operation to an individual
variable -- first assigning `0` to the variable, then applying arithmetic operations to it.

```ion
let value = 0
let value += 5
let value -= 2
let value *= 3
let value /= 2
```


## Multiple Assignments

It's also possible to perform a mathematical operation to multiple variables. Each variable will be
designated with a paired value.

```ion
let a b = 5 5
let a b += 3 2
let a b -= 1 1
echo $a $b
```

This will output the following:

```
7 6
```
