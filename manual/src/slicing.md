# Slicing Syntax

Ion supports a universal syntax for slicing strings and arrays. For maximum language support,
strings are sliced and indexed by graphemes. Arrays are sliced and indexed by their elements.
Slicing uses the same **[]** characters as arrays, but the shell can differentation between
a slice and an array based on the placement of the characters (immediately after an expansion).

**NOTE:** It's important to note that indexes count from 0, as in most other languages.

## Exclusive Range

The exclusive syntax will grab all values starting from the first index, and ending on
the Nth element, where N is the last index value. The Nth element's ID is always one
less than the Nth value.

```ion
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

```ion
$ let array = [{1...10}]
$ echo @array[1...5]
> 1 2 3 4 5 6
```

## Process Expansions Also Support Slicing

Variables aren't the only elements that support slicing. Process expansions also support slicing.

```ion
$ echo $(cat file)[..10]
$ echo @(cat file)[..10]
```
