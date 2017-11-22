# Method Expansions

There are two forms of methods within Ion: array methods, and string methods. Array methods are
methods which return arrays, and string methods are methods which return strings. The distinction
is made between the two by the sigil that is invoked when calling a method. For example, if the
method is denoted by the `$` sigil, then it is a string method. Otherwise, if it is denoted by the
`@` sigil, then it is an array method. Example as follows:

```ion
echo $method_name(variable)
for elem in @method_name(variable); echo $elem; end
```

Methods are executed at the same time as other expansions, so this leads to a performance
optimization when combining methods with other methods or expansions. Ion includes a number of these
methods for common use cases, but it is possible to create and/or install new methods to enhance the
functionality of Ion. Just ensure that systems executing your Ion scripts that require those plugins
are equipped with and have those plugins enabled. If you have ideas for useful methods that would
be worth including in Ion by default, feel free to open a feature request to discuss the idea.

## Methods Support Inline Expressions

So we heard that you like methods, so we put methods in your methods. Ion methods accept taking
expressions as their arguments -- both for the input parameter, and any supplied arguments to
control the behavior of the method.

```ion
echo $method($(cmd...), arg)

let string_var = "value in variable"
echo $method(string_var)

echo $method("actual value", arg)
```

## Overloaded Methods

Some methods may also perform different actions when supplied a different type. The `$len()` method,
for example, will report the number of graphemes within a string, or the number of elements within
an array. Ion is able to determine which of the two were provided based on the first character
in the expression. Quoted expressions, and expressions with start with `$`, are strings; whereas
expressions that start with either `[` or `@` are treated as arrays.

```ion
echo $len("a string")
echo $len([1 2 3 4 5])
```

## Method Arguments

Some methods may have their behavior tweaked by supplying some additional arguments. The `@split()`
method, for example, may be optionally supplied a pattern for splitting. At the moment, a comma
is used to specify that arguments are to follow the input, but each argument supplied after that
is space-delimited.

```ion
for elem in @split("some space-delimited values"); echo $elem; end
for elem in @split("some, comma-separated, values", ", "); echo $elem; end
```

## String Methods

The following are the currently-supported string methods:

- [ends_with](#ends_with)
- [contains](#contains)
- [starts_with](#starts_with)
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

### ends_with

Defaults to string variables. When supplied with a pattern, it will return one if the string
ends with it. Zero otherwise.

#### Examples

```ion
echo $ends_with("FOOBAR", "BAR")
echo $ends_with("FOOBAR", "FOO")
```

#### Output

```
1
0
```

### contains

Defaults to string variables. When supplied with a pattern, it will return one if the string
contains with it. Zero otherwise.

#### Examples

```ion
echo $contains("FOOBAR", "OOB")
echo $contains("FOOBAR", "foo")
```

#### Output

```
1
0
```

### starts_with

Defaults to string variables. When supplied with a pattern, it will return one if the string
starts with it. Zero otherwise.

#### Examples

```ion
echo $starts_with("FOOBAR", "FOO")
echo $starts_with("FOOBAR", "BAR")
```

#### Output

```
1
0
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
file name portion of the complete filename. IE: `/parent/filename.ext` -> `filename`.

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

### find

Defaults to string variables. When given an string, it returns the first index in which that
string appears. It returns `-1` if it isn't contained.

#### Examples

```ion
echo $find("FOOBAR", "OB")
echo $find("FOOBAR", "ob")
```

#### Output

```
2
-1
```

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

### regex_replace

Defaults to string variables. Equivalent to `replace`, but the first argument will be treated
as a regex.

#### Examples

```ion
echo $regex_replace("FOOBAR", "^F" "f")
echo $regex_replace("FOOBAR", "^f" "F")
```

#### Output

```
fOOBAR
FOOBAR
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

### escape

Defaults to string variables. Escapes the content of the string.

#### Example

```ion
let line = " Mary   had\ta little  \n\t lamb\t"
echo $escape($line)
```

#### Output

```
 Mary   had\\ta little  \\n\\t lamb\\t
```

### unescape

Defaults to string variables. Unescapes the content of the string.

#### Example

```
let line = " Mary   had\ta little  \n\t lamb\t"
echo $unescape($line)
```

#### Output

```
 Mary   had	a little  
	 lamb
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
