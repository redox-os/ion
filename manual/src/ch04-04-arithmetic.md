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
- [ ] Powers (not stabilized yet: **^**; subject to change to **\*\***)


```ion
let value = 0
let value += 5
let value -= 2
let value *= 3
let value /= 2
```

