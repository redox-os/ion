## String Methods

The following are the currently-supported string methods:

- [basename](#basename)
- [extension](#extension)
- [filename](#filename)
- [join](#join)
- [find](#find)
- [len](#len)
- [len_bytes](#len_bytes)
- [parent](#parent)
- [repeat](#repeat)
- [replace](#replace)
- [replacen](#replacen)
- [regex_replace](#regex_replace)
- [reverse](#reverse)
- [to_lowercase](#to_lowercase)
- [to_uppercase](#to_uppercase)
- [escape](#escape)
- [unescape](#unescape)
- [or](#or)

### basename
Defaults to string variables. When given a path-like string as input, this will return the
basename (complete filename, extension included). IE: `/parent/filename.ext` -> `filename.ext`
```sh
{{#include ../../../tests/string_methods.ion:basename}}
```
```txt
{{#include ../../../tests/string_methods.out:basename}}
```

### extension
Defaults to string variables. When given a path-like string as input, this will return the
extension of the complete filename. IE: `/parent/filename.ext` -> `ext`.
```sh
{{#include ../../../tests/string_methods.ion:extension}}
```
```txt
{{#include ../../../tests/string_methods.out:extension}}
```

### filename
Defaults to string variables. When given a path-like string as input, this will return the
file name portion of the complete filename. IE: `/parent/filename.ext` -> `filename`.
```sh
{{#include ../../../tests/string_methods.ion:filename}}
```
```txt
{{#include ../../../tests/string_methods.out:filename}}
```

### join
Defaults to array variables. When given an array as input, the join string method will concatenate
each element in the array and return a string. If no argument is given, then those elements will
be joined by a single space. Otherwise, each element will be joined with a given pattern.
```sh
{{#include ../../../tests/string_methods.ion:join}}
```
```txt
{{#include ../../../tests/string_methods.out:join}}
```

### find
Defaults to string variables. When given an string, it returns the first index in which that
string appears. It returns `-1` if it isn't contained.
```sh
{{#include ../../../tests/string_methods.ion:find}}
```
```txt
{{#include ../../../tests/string_methods.out:find}}
```

### len
Defaults to string variables. Counts the number of graphemes in the output. If an array expression
is supplied, it will print the number of elements in the array.
```sh
{{#include ../../../tests/string_methods.ion:len}}
```
```txt
{{#include ../../../tests/string_methods.out:len}}
```

### len_bytes
Defaults to string variables. Similar to the `len` method, but counts the number of actual bytes
in the output, not the number of graphemes.
```sh
{{#include ../../../tests/string_methods.ion:len_bytes}}
```
```txt
{{#include ../../../tests/string_methods.out:len_bytes}}
```

### parent
Defaults to string variables. When given a path-like string as input, this will return the
parent directory's name. IE: `/root/parent/filename.ext` -> `/root/parent`
```sh
{{#include ../../../tests/string_methods.ion:parent}}
```
```txt
{{#include ../../../tests/string_methods.out:parent}}
```

### repeat
Defaults to string variables. When supplied with a number, it will repeat the input N
amount of times, where N is the supplied number.
```sh
{{#include ../../../tests/string_methods.ion:repeat}}
```
```txt
{{#include ../../../tests/string_methods.out:repeat}}
```

### replace
Defaults to string variables. Given a pattern to match, and a replacement to replace each match
with, a new string will be returned with all matches replaced.
```sh
{{#include ../../../tests/string_methods.ion:replace}}
```
```txt
{{#include ../../../tests/string_methods.out:replace}}
```

### replacen
Defaults to string variables. Equivalent to `replace`, but will only replace the first N amount
of matches.
```sh
{{#include ../../../tests/string_methods.ion:replacen}}
```
```txt
{{#include ../../../tests/string_methods.out:replacen}}
```

### regex\_replace
Defaults to string variables. Equivalent to `replace`, but the first argument will be treated
as a regex.

**PS:** By default, unicode support will be disabled to trim the size of Ion. Add the "unicode" flag to enable it.
```sh
{{#include ../../../tests/string_methods.ion:regex_replace}}
```
```txt
{{#include ../../../tests/string_methods.out:regex_replace}}
```

### reverse
Defaults to string variables. Simply returns the same string, but with each grapheme displayed
in reverse order.
```sh
{{#include ../../../tests/string_methods.ion:reverse}}
```
```txt
{{#include ../../../tests/string_methods.out:reverse}}
```

### to_lowercase
Defaults to string variables. All given strings have their characters converted to an
lowercase equivalent, if an lowercase equivalent exists.
```sh
{{#include ../../../tests/string_methods.ion:to_lowercase}}
```
```txt
{{#include ../../../tests/string_methods.out:to_lowercase}}
```

### to_uppercase
Defaults to string variables. All given strings have their characters converted to an
uppercase equivalent, if an uppercase equivalent exists.
```sh
{{#include ../../../tests/string_methods.ion:to_uppercase}}
```
```txt
{{#include ../../../tests/string_methods.out:to_uppercase}}
```

### escape

Defaults to string variables. Escapes the content of the string.
```sh
{{#include ../../../tests/string_methods.ion:escape}}
```
```txt
{{#include ../../../tests/string_methods.out:escape}}
```

### unescape
Defaults to string variables. Unescapes the content of the string.
```sh
{{#include ../../../tests/string_methods.ion:unescape}}
```
```txt
{{#include ../../../tests/string_methods.out:unescape}}
```

### or
Defaults to string variables. Fallback to a given value if the variable is not defined or is an empty string.
```sh
{{#include ../../../tests/string_methods.ion:or}}
```
```txt
{{#include ../../../tests/string_methods.out:or}}
```
