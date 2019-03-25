# Ranges & Slicing Syntax

Ion supports a universal syntax for slicing strings and arrays. For maximum language support,
strings are sliced and indexed by graphemes. Arrays are sliced and indexed by their elements.
Slicing uses the same **[]** characters as arrays, but the shell can differentation between
a slice and an array based on the placement of the characters (immediately after an expansion).

**NOTE:** It's important to note that indexes count from 0, as in most other languages.

## Exclusive Range

The exclusive syntax will grab all values starting from the first index, and ending on
the Nth element, where N is the last index value. The Nth element's ID is always one
less than the Nth value.

```sh
$ let array = [{1...10}]
$ echo @array[0..5]
> 1 2 3 4 5

$ echo @array[..5]
> 1 2 3 4 5

$ let string = "hello world"
$ echo $string[..5]
> hello
$ echo $string[6..]
> world
```

## Inclusive Range

When using inclusive ranges, the end index does not refer to the Nth value, but the actual index ID.

```sh
$ let array = [{1...10}]
$ echo @array[1...5]
> 1 2 3 4 5 6
```

## Descending Ranges

Ranges do not have to always be specified in ascending order. Descending ranges are also
supported. However, at this time you cannot provide an descending range as an index to an array.

```sh
$ echo {10...1}
> 10 9 8 7 6 5 4 3 2 1
$ echo {10..1}
> 10 9 8 7 6 5 4 3 2
```

## Negative Values Supported

Although this will not work for arrays, you may supply negative values with ranges to create
negative values in a range of numbers.i

```sh
$ echo {-10...10}
> -10 -9 -8 -7 -6 -5 -4 -3 -2 -1 0 1 2 3 4 5 6 7 8 9 10
```

## Stepping Ranges

Stepped ranges are also supported.

### Stepping Forward w/ Brace Ranges

Brace ranges support a syntax similar to Bash, where the starting index is supplied, followed by
two periods and a stepping value, followed by either another two periods or three periods, then
the end index.

```sh
$ echo {0..3...12}
> 0 3 6 9 12
$ echo {1..2..12}
> 0 3 6 9
$ let array = [{1...30}]
```

### Stepping Forward w/ Array Slicing

Array slicing, on the other hand, uses a more Haskell-ish syntax, whereby instead of specifying
the stepping with two periods, it is specified with a comma.

```sh
$ let array = [{0...30}]
$ echo @array[0,3..]
> 0 3 6 9 12 15 18 21 24 27 30
```

## Stepping In Reverse w/ Brace Ranges

Brace ranges may also specify a range that descends in value, rather than increases.

```sh
$ echo {10..-2...-10}
> 10 8 6 4 2 0 -2 -4 -6 -8 -10
$ echo {10..-2..-10}
> 10 8 6 4 2 0 -2 -4 -6 -8
```

## Stepping In Reverse w/ Array Slicing

Arrays may also be sliced in reverse order using the same syntax as for reverse. Of course,
negative values aren't allowed here, so ensure that the last value is never less than 0.
Also note that when a negative stepping is supplied, it is automatically inferred for the
end index value to be 0 when not specified.

```sh
$ let array = [{0...30}]
$ echo @array[30,-3..]
> 30 27 24 21 18 15 12 9 6 3 0
```

## Process Expansions Also Support Slicing

Variables aren't the only elements that support slicing. Process expansions also support slicing.

```sh
$ echo $(cat file)[..10]
$ echo @(cat file)[..10]
```
