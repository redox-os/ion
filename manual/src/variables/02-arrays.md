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

## Create a new array

Arguments enclosed within brackets are treated as elements within an array.

```sh
let array = [ one two 'three four' ]
```

## Indexing into an array

Values can be fetched from an array via their position in the array as the index.

```
let array = [ 1 2 3 4 5 6 7 8 9 10 ]
echo @array[0]
echo @array[5..=8]
```

## Copy array into a new array

Passing an array within brackets enables performing a deep copy of that array.

```sh
let array_copy = [ @array ]
```

## Array join

This will join each element of the array into a string, adding spaces between each element.

```sh
let array = [ hello world ]
let other_array = [ this is the ion ]
let array = [ @array @other_array shell ]
let as_string = @array
echo @array
echo $array
```

```
hello world this is the ion shell
hello world this is the ion shell
```

## Array concatenation

The `++=` and `::=` operators can be used to efficiently concatenate an array in-place.

```sh
let array = [1 2 3]
let array ++= [5 6 7]
let array ::= 0
echo @array
```

```
0 1 2 3 5 6 7
```

## Expand array as arguments to a command

Arrays are useful to pass as arguments to a command. Each element will be expanded as an
individual argument, if any arguments exist.

```sh
let args = [-l -a --color]
ls @args
```
