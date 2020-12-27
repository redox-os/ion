# Method Expansions

There are two forms of methods within Ion: [string methods](06-stringmethods.md) and [array methods](07-arraymethods.md). Array methods are
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
