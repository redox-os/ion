# Method Expansions

There are two forms of methods within Ion: array methods, and string methods. Array methods are
methods which return arrays, and string methods are methods which return strings. The distinction
is made between the two by the sigil that is invoked when calling a method. For example, if the
method is denoted by the `$` sigil, then it is a string method. Otherwise, if it is denoted by the
`@` sigil, then it is an array method. Example as follows:

```sh
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

```sh
echo $method($(cmd...) arg)

let string_var = "value in variable"
echo $method(string_var)

echo $method("actual value" arg)
```

## Overloaded Methods

Some methods may also perform different actions when supplied a different type. The `$len()` method,
for example, will report the number of graphemes within a string, or the number of elements within
an array. Ion is able to determine which of the two were provided based on the first character
in the expression. Quoted expressions, and expressions with start with `$`, are strings; whereas
expressions that start with either `[` or `@` are treated as arrays.

```sh
echo $len("a string")
echo $len([1 2 3 4 5])
```

## Method Arguments

Some methods may have their behavior tweaked by supplying some additional arguments. The `@split()`
method, for example, may be optionally supplied a pattern for splitting.

```sh
for elem in @split("some space-delimited values"); echo $elem; end
for elem in @split("some, comma-separated, values" ", "); echo $elem; end
```

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

#### Examples

```sh
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

```sh
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

```sh
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

```sh
let array = [1 2 3 4 5]
echo $join(array)
echo $join(array ", ")
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

```sh
echo $find("FOOBAR" "OB")
echo $find("FOOBAR" "ob")
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

```sh
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

```sh
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

```sh
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

```sh
echo $repeat("abc, " 3)
```

#### Output

```
abc, abc, abc,
```

### replace

Defaults to string variables. Given a pattern to match, and a replacement to replace each match
with, a new string will be returned with all matches replaced.

#### Examples

```sh
let input = "one two one two"
echo $replace(input one 1)
echo $replace($replace(input one 1) two 2)
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

```sh
let input = "one two one two"
echo $replacen(input "one" "three" 1)
echo $replacen(input "two" "three" 2)
```

#### Output

```
three two one two
one three one three
```

### regex\_replace

Defaults to string variables. Equivalent to `replace`, but the first argument will be treated
as a regex.

**PS:** By default, unicode support will be disabled to trim the size of Ion. Add the "unicode" flag to enable it.

#### Examples

```sh
echo $regex_replace("bob" "^b" "B")
echo $regex_replace("bob" 'b$' "B")
```

#### Output

```
Bob
boB
```

### reverse

Defaults to string variables. Simply returns the same string, but with each grapheme displayed
in reverse order.

#### Examples

```sh
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

```sh
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

```sh
echo $to_uppercase("foobar")
```

#### Output

```
FOOBAR
```

### escape

Defaults to string variables. Escapes the content of the string.

#### Example

```sh
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

### or

Defaults to string variables. Fallback to a given value if the variable is not defined or is an empty string.

#### Example

```
echo $or($unknown_variable "Fallback")
let var = 42
echo $or($var "Not displayed")
```

#### Output

```
Fallback
42
```

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

#### Examples

```sh
for line in @lines($unescape("first\nsecond\nthird"))
    echo $line
end
```

#### Output

```
first
second
third
```

### split

Defaults to string variables. The supplied string will be split according to a pattern specified
as an argument in the method. If no pattern is supplied, then the input will be split by
whitespace characters. Useful for splitting simple tabular data.

#### Examples

```sh
for data in @split("person, age, some data" ", ")
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

### split_at

Defaults to string variables. The supplied string will be split in two pieces, from the index specified in the second argument.

#### Examples

```
echo @split_at("FOOBAR" "3")
echo @split_at("FOOBAR")
echo @split_at("FOOBAR" "-1")
echo @split_at("FOOBAR" "8")
```

#### Output

```
FOO BAR
ion: split_at: requires an argument
ion: split_at: requires a valid number as an argument
ion: split_at: value is out of bounds
```

### bytes

Defaults to string variables. Returns an array where the given input string is split by bytes and
each byte is displayed as their actual 8-bit number.

#### Examples

```sh
echo @bytes("abc")
```

#### Output

```
97 98 99
```

### chars

Defaults to string variables. Returns an array where the given input string is split by chars.

#### Examples

```sh
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

```sh
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

### reverse

Defaults to array variables. Returns a reversed copy of the input array.

#### Examples

```sh
echo @reverse([1 2 3])
```

#### Output

```
3 2 1
```

