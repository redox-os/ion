# Array Variables

The **[]** syntax in Ion denotes that the contents within should be parsed as an
array expression. 
On using `let` keyword for array variables, all array arguments must be wrapped within the **[]** syntax. Otherwise it will be coerced into a space-separated string.
This design decision was made due to the possibility of an expanded array with one element 
being interpreted as a string.

Once created, you may call an array variable in the same manner as a string variable, but you
must use the **@** sigil instead of **$**. When expanded, arrays will be expanded into multiple
arguments. Hence it is possible to use arrays to set multiple arguments in commands. 

**NOTE** If an array is double quoted, it will be coerced into a string. This behavior is equivalent to invoking the `$join(array)` method.

**NOTE**: Brace expansions also create arrays.

## Create a new array
Arguments enclosed within brackets are treated as elements within an array.
```sh
{{#include ../../../tests/array_vars.ion:create_array}}
```
```txt
{{#include ../../../tests/array_vars.out:create_array}}
```

## Indexing into an array
Values can be fetched from an array via their position in the array as the index.
```sh
{{#include ../../../tests/array_vars.ion:index_array}}
```
```txt
{{#include ../../../tests/array_vars.out:index_array}}
```

## Copy array into a new array
Passing an array within brackets enables performing a deep copy of that array.
```sh
{{#include ../../../tests/array_vars.ion:array_copy}}
```
```txt
{{#include ../../../tests/array_vars.out:array_copy}}
```

## Array join
This will join each element of the array into a string, adding spaces between each element.
```sh
{{#include ../../../tests/array_vars.ion:array_join}}
```
```txt
{{#include ../../../tests/array_vars.out:array_join}}
```

## Array concatenation and variable stripping
The `++=` and `::=` operators can be used to efficiently concatenate an array in-place.
```sh
{{#include ../../../tests/array_vars.ion:array_concat_var_strip}}
```
```txt
{{#include ../../../tests/array_vars.out:array_concat_var_strip}}
```

## Practical array usage
Passing arrays as command arguments and capturing output of commands as arrays is useful.
```sh
{{#include ../../../tests/array_vars.ion:practical_array}}
```
```txt
{{#include ../../../tests/array_vars.out:practical_array}}
```
