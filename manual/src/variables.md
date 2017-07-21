# Variable Assignments

The `let` builtin is used to create local variables within the shell, and apply basic arithmetic
to variables. The `export` keyword may be used to do the same for the creation of external
variables. Variables cannot be created the POSIX way, as the POSIX way is awkard to read/write
and parse.

## String Variables

Using the `let` builtin, a string can easily be created by specifying the name, and an expression
that will be evaluated before assigning it to that variable.


```ion
let git_branch = $(git rev-parse --abbrev-ref HEAD ^> /dev/null)
```

To call a string variable, you may utilize the **$** sigil along with the name of the variable. For more information on expansions, see the expansions section of this manual.

```ion
echo $git_branch
```

## Tuple Assignments

Ion also supports assigning multiple variables at once, which can increase readability and save
some precious CPU cycles. The general trend is that the less statements that you execute, the
faster your scripts will execute, but there are some exceptions to the rule -- see the general
tips in the miscellanious section. In addition to assigning multiple variables, this can also
be used to swap variables.

```ion
let a b = 1 2
let a b = [1 2]
let a b = [$b $a]
```

Do note, however, that if you supply too many values, they will be ignored.

```ion
$ let a b = 1 2 3
$ echo $a $b
> 1 2
```

## Dropping String Variables

The `drop` command may be used to drop string variables.

```ion
let variable = "testing"
echo $variable
drop variable
echo $variable
```

## Array Variables

The **[]** syntax in Ion is utilized to denote that the contents within should be parsed as an
array expression. Array variables are also created using the same `let` keyword, but `let` makes
the distinction between a string and an array by additionally requiring that all array arguments
are wrapped within the **[]** syntax. If an array is supplied to `let` that is not explicitly
declared as an array, then it will be coerced into a space-separated string. This design decision
was made due to the possibility of an expanded array with one element being interpreted as a
string.

Once created, you may call an array variable in the same manner as a string variable, but you
must use the **@** sigil instead of **$**. When expanded, arrays will be expanded into multiple
arguments, so it is possible to use arrays to set multiple arguments in commands. Do note, however,
that if an array is double quoted, it will be coerced into a string, which is a behavior that
is equivalent to invoking the `$join(array)` method.

**NOTE**: Brace expansions also create arrays.

```ion
let array = [ one two 'three four' ]
let array_copy = [ @array ]
let as_string = @array
let args = [-l -a --color]
ls @args
```

## Dropping Array Variables

The `drop -a` command will drop array variables from the shell.

```sh
let array = [one two three]
echo @array
drop -a array
echo @array
```

## Let Arithmetic

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

## Exporting Variables

The `export` builtin operates identical to the `let` builtin, but it does not support arrays,
and variables are exported to the OS environment.

```ion
export GLOBAL_VAL = "this"
```
