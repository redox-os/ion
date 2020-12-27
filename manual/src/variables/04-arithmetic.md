# Let Arithmetic

Ion supports applying some basic arithmetic, one operation at a time, to string variables. To
specify to `let` to perform some arithmetic, designate the operation immediately before **=**.
Operators currently supported are:

- [x] Add (**+**)
- [x] Subtract (**-**)
- [x] Multiply (**\***)
- [x] Divide (**/**)
- [x] Integer Divide (**//**)
- [ ] Modulus (**%**)
- [x] Powers  (**\*\***)

## Individual Assignments
The following examples are a demonstration of applying a mathematical operation to an individual
variable.
```sh
{{#include ../../../tests/arithmetic_vars.ion:individual_assignments}}
```
```txt
{{#include ../../../tests/arithmetic_vars.out:individual_assignments}}
```

## Multiple Assignments
It's also possible to perform a mathematical operation to multiple variables. Each variable will be
designated with a paired value.
```sh
{{#include ../../../tests/arithmetic_vars.ion:multiple_assignments}}
```
```txt
{{#include ../../../tests/arithmetic_vars.out:multiple_assignments}}
```
