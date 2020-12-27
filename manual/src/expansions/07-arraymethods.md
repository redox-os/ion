## Array Methods
The following are the currently-supported array methods.
- [lines](#lines)
- [split](#split)
- [split_at](#split_at)
- [bytes](#bytes)
- [chars](#chars)
- [graphemes](#graphemes)
- [reverse](#reverse)

### lines
Defaults to string variables. The supplied string will be split into one string per line in the input argument.
This is equivalent to `@split(value '\n')`.
```sh
{{#include ../../../tests/array_methods.ion:lines}}
```
```txt
{{#include ../../../tests/array_methods.out:lines}}
```

### split
The supplied string will be split according to a pattern specified
as an argument in the method. If no pattern is supplied, then the input will be split by
whitespace characters. Useful for splitting simple tabular data.
```sh
{{#include ../../../tests/array_methods.ion:split}}
```
```txt
{{#include ../../../tests/array_methods.out:split}}
```

### split_at
Defaults to string variables. The supplied string will be split in two pieces, from the index specified in the second argument.
```sh
{{#include ../../../tests/array_methods.ion:split_at}}
```
```txt
{{#include ../../../tests/array_methods.out:split_at}}
```

### bytes
Defaults to string variables. Returns an array where the given input string is split by bytes and
each byte is displayed as their actual 8-bit number.
```sh
{{#include ../../../tests/array_methods.ion:bytes}}
```
```txt
{{#include ../../../tests/array_methods.out:bytes}}
```

### chars
Defaults to string variables. Returns an array where the given input string is split by chars.
```sh
{{#include ../../../tests/array_methods.ion:chars}}
```
```txt
{{#include ../../../tests/array_methods.out:chars}}
```

### graphemes
Defaults to string variables. Returns an array where the given input string is split by graphemes.
```sh
{{#include ../../../tests/array_methods.ion:graphemes}}
```
```txt
{{#include ../../../tests/array_methods.out:graphemes}}
```

### reverse
Defaults to array variables. Returns a reversed copy of the input array.
```sh
{{#include ../../../tests/array_methods.ion:reverse}}
```
```txt
{{#include ../../../tests/array_methods.out:reverse}}
```
