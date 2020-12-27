# String Variables
We can evaluate expressions to assign their result to the variable and print with with **$** sigil.
Read the chapter expansions for more information about the expansion behavior.
```sh
{{#include ../../../tests/string_vars.ion:string_variables}}
```
```txt
{{#include ../../../tests/string_vars.out:string_variables}}
```

## Slicing a string.
Strings can be sliced in Ion using a range.
```sh
{{#include ../../../tests/string_vars.ion:string_slicing}}
```
```txt
{{#include ../../../tests/string_vars.out:string_slicing}}
```

## String concatenation
The `++=` and `::=` operators can be used to efficiently concatenate a string in-place.
```sh
{{#include ../../../tests/string_vars.ion:string_concatenation}}
```
```txt
{{#include ../../../tests/string_vars.out:string_concatenation}}
```
