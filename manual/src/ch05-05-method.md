# Method Expansions

There are two forms of methods within Ion: array and string methods. Array methods are methods which return arrays,
and string methods are methods which return strings. Invoking an array method requires denoting that the method
is an array method with the '@' character, whereas using '$' for string methods -- same as process and variable
expansions. The general syntax of a method is '<sigil><name_of_method>(<input>, <arg1> <arg2> <args>...)'.

Methods are executed at the same time as other expansions, so this leads to a performance optimization when combining
methods with other methods or expansions. Ion includes a number of these methods for common use cases, but it is
possible to create and/or install new methods to enhance the functionality of Ion. Just ensure that systems executing
your Ion scripts that require those plugins are equipped with and have those plugins enabled.

## Methods Support Inline Expressions

When parsing the input element, if the element is a literal that is not defined as an expression, the given value
will be treated as the name of a variable that corresponds to the default input type for that method. Basically,
`array` would indicate to the parser that we are calling a variable of that name, whereas `"array"`, `'array'`,
`@array`, `$array`, `$(cmd)`, etc. would indicate to the parser that we are evaluating and using the expression's
output as the input.

```ion
echo $method($(cmd...), arg)

let string_var = "value in variable"
echo $method(string_var)

echo $method("actual value", arg)
```

## String Methods

The following are the currently-supported string methods:

- [basename](#basename)
- [extension](#extension)
- [filename](#filename)
- [join](#join)
- [len](#len)
- [len_bytes](#len_bytes)
- [parent](#parent)
- [repeat](#repeat)
- [replace](#replace)
- [replacen](#replacen)
- [reverse](#reverse)
- [to_lowercase](#to_lowercase)
- [to_uppercase](#to_uppercase)

### len

Defaults to string variables. Counts the number of graphemes in the output. If an array expression
is supplied, it will print the number of elements in the array.

#### Examples

```ion
echo $len("foobar")
echo $len("❤️")
echo $len([one two three four])
```

#### Output

```
6
1
4
```

### len_bytes

Defaults to string variables. Similar to the `len` method, but counts the number of actual bytes
in the output, not the number of graphemes.

#### Examples

```ion
echo $len_bytes("foobar")
echo $len_bytes("❤️")
```

#### Output

```
6
6
```

### parent

Defaults to string variables. When given a path-like string as input, this will return the
parent directory's name. IE: `/root/parent/filename.ext` -> `/root/parent`

#### Examples

```ion
echo $parent("/root/parent/filename.ext")
```

#### Output

```
/root/parent
```

### repeat

Defaults to string variables. When supplied with a number, it will repeat the input N
amount of times, where N is the supplied number.

#### Examples

```ion
echo $repeat("abc, ", 3)
```

#### Output

```
abc, abc, abc
```

### basename

Defaults to string variables. When given a path-like string as input, this will return the
basename (complete filename, extension included). IE: `/parent/filename.ext` -> `filename.ext`

#### Examples

```ion
echo $basename("/parent/filename.ext")
```

#### Output

```
filename.ext
```

### extension

Defaults to string variables. When given a path-like string as input, this will return the
extension of the complete filename. IE: `/parent/filename.ext` -> `ext`.

#### Examples

```ion
echo $extension("/parent/filename.ext")
```

#### Output

```
ext
```

### filename

Defaults to string variables. When given a path-like string as input, this will return the
file stem of the complete filename. IE: `/parent/filename.ext` -> `filename`.

#### Examples

```ion
echo $filename("/parent/filename.ext")
```

#### Output

```
filename
```

### join

Defaults to array variables. When given an array as input, the join string method will concatenate
each element in the array and return a string. If no argument is given, then those elements will
be joined by a single space. Otherwise, each element will be joined with a given pattern.

#### Examples

```ion
let array = [1 2 3 4 5]
echo $join(array)
echo $join(array, ", ")
```

#### Output

```
1 2 3 4 5
1, 2, 3, 4, 5
```

###

### replace

Defaults to string variables. Given a pattern to match, and a replacement to replace each match
with, a new string will be returned with all matches replaced.

#### Examples

```ion
let input = "one two one two"
echo $replace(input, one 1)
echo $replace($replace(input, one 1), two 2)
```

#### Output

```
1 two 1 two
1 2 1 2
```

### replacen

Defaults to string variables. Equivalent to `replace`, but will only replace the first N amount
of matches.

#### Examples

```ion
let input = "one two one two"
echo $replacen(input, "one" "three" 1)
echo $replacen(input, "two" "three" 2)
```

#### Output

```
three two one two
one three one three
```

### reverse

Defaults to string variables. Simply returns the same string, but with each grapheme displayed
in reverse order.

#### Examples

```ion
echo $reverse("foobar")
```

#### Output

```
raboof
```

### to_lowercase

Defaults to string variables. All given strings have their characters converted to an
lowercase equivalent, if an lowercase equivalent exists.

#### Examples

```ion
echo $to_lowercase("FOOBAR")
```

#### Output

```
foobar
```

### to_uppercase

Defaults to string variables. All given strings have their characters converted to an
uppercase equivalent, if an uppercase equivalent exists.

#### Examples

```ion
echo $to_uppercase("foobar")
```

#### Output

```
FOOBAR
```

## Array Methods

The following are the currently-supported array methods.

- [split](#split)
- [bytes](#bytes)
- [chars](#chars)
- [graphemes](#graphemes)

### split

Defaults to string variables. The supplied string will be split according to a pattern specified
as an argument in the method. If no pattern is supplied, then the input will be split by
whitespace characters. Useful for splitting simple tabular data.

#### Examples

```ion
for data in @split("person, age, some data", ", ")
    echo $data
end

for data in @split("person age data")
    echo $data
end
```

#### Output

```
person
age
some data
person
age
data
```

### bytes

Defaults to string variables. Returns an array where the given input string is split by bytes and
each byte is displayed as their actual 8-bit number.

#### Examples

```ion
echo @bytes("foobar")
```

#### Output

```
102 111 111 98 97 114
```

### chars

Defaults to string variables. Returns an array where the given input string is split by chars.

#### Examples

```ion
for char in @chars("foobar")
    echo $char
end
```

#### Output

```
f
o
o
b
a
r
```

### graphemes

Defaults to string variables. Returns an array where the given input string is split by graphemes.

#### Examples

```ion
for grapheme in @graphemes("foobar")
    echo $grapheme
end
```

#### Output

```
f
o
o
b
a
r
```
