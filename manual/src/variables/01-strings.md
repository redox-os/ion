# String Variables

Using the `let` builtin, a string can easily be created by specifying the name, and an expression
that will be evaluated before assigning it to that variable.


```sh
let git_branch = $(git rev-parse --abbrev-ref HEAD ^> /dev/null)
```

## Calling a string variable.

To call a string variable, you may utilize the **$** sigil along with the name of the variable. For more information on expansions, see the expansions section of this manual.

```sh
echo $git_branch
```

## Slicing a string.

Strings can be sliced in Ion using a range.

```sh
let foo = "Hello, World"
echo $foo[..5]
echo $foo[7..]
echo $foo[2..9]
```

## String concatenation

The `++=` and `::=` operators can be used to efficiently concatenate a string in-place.

```sh
let string = "ello"
let string ::= H
let string ++= ", world!"
echo $string
```

```
Hello, world!
```
