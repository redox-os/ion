# Array Variables

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
