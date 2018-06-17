- Feature Name: variable_assignment
- Start Date: 2018-06-12
- RFC PR: N/A
- Ion Issues: #777

# Summary
[summary]: #summary

Variables are assigned using the `let` keyword. A `let` statement will consist of any number of
variable keys on the left side of the statement, an assignment operator, and a collection of
associated values on the right side of the statement. Variables may be optionally-typed, which
will ensure that values fit the expected criteria for the variables.

# Motivation
[motivation]: #motivation

POSIX shells have limited flexibility in how they accomplish variable assignments:

- They require that the first word of every statement is scanned for an `=` operator.
- They are intolerant of white space characters within a statement.
- They are unable to support type-checked assignments.
- Nor are they able to handle tuple assignments.
- Or can they be used to assign arrays, maps, and nested structures.

It seems strange for shells to assign variables in the way that POSIX requires. Shells treat
the first word of every statement as the command to initiate, and all words that follow as
arguments to that command. It is therefore natural for a keyword to be used to set the
intention of a statement, and that keyword will be capable of doing all of these things.

`let` has been chosen as the keyword for two reasons:

- It is a terse keyword that describes intent in three characters
- Ion is written in Rust, and Rust uses the `let` keyword for assignments

# Detailed design
[design]: #detailed-design

## String Assignments
[strings]: #strings

The basic statement structure for a string variable assignment is as follows:

```
let KEY = VALUE
```

`let` initiates the intent to assign variables, `KEY` is the variable to be assigned, `=` is the
assignment operator to use, and `VALUE` will be assigned to `KEY`.

## Array Assignments
[arrays]: #arrays

Arrays are assigned using `[]` brackets. Each word contained within is treated as an individual
argument within the array. The reason that `[]` is required to create an array is due to
difficulties in discerning which arguments are to be assigned as a string, and which are to be
grouped into an array. This will not only make it easier read, but enables us to implicitly assign
values to multiple keys, even if some of the keys are strings, and others arrays.

```
$ let FOO = [FOO BAR BAZ]
$ echo @FOO
FOO BAR BAZ
```

### Array Index Assignment
[array-index-assignment]: #array-index-assignment

When assigning directly to an index in an array, if one value is supplied, that value will
be assigned to that index. If an array of values is supplied, the array will be inserted
at that location.

```
$ let FOO = [4 5 6]
$ let FOO[0] = [1 2 3]
$ echo @FOO
1 2 3 4 5 6
```

### Array Index Range Assignment
[array-index-range-assignment]: #array-index-range-assignment

It is also possible to assign an array to replace a range of values. The array being assigned to
that region may be smaller or larger than the region it is replacing.

```
$ let FOO = [1 2 3 4]
$ let FOO[..1] = [4 5 6]
$ let FOO[2..3] = [7]
$ echo @FOO
4 5 6 7
```

## Copying Values
[copy]: #copy

A value can be copied by using the variable as the value for the assignment. String variables
may be invoked with the `$` sigil, while array variables may be called with the `@` sigil. The
`[]` is still required to create an array from an array, otherwise the array will be joined
together as a string.

```
let STRING_COPY = $STRING
let ARRAY_COPY = [ @ARRAY ]
```

The reason that `@ARRAY` requires to be within brackets is the same as mentioned before. It is
possible that a user may want to store an array into a string, or combine an array with strings
or other arrays. Requiring arrays to be explicitly declared with brackets simplifies the logic
required to implement assignment parsing. This could also open the door to possible future
feature additions, such as nested arrays.

## Tuple Assignments
[tuples]: #tuples

Multiple keys may be on the left side of the statement, and multiple values may be on the right.
Values are expanded in parallel, thus making it easy to swap variables in place.

```
$ let FOO BAR BAZ = YOU USE ION
$ let FOO BAZ = $BAZ $FOO
$ echo $FOO $BAR $BAZ
ION USE YOU
```

## Intermixing Types
[implicit-tuple]: #implicit-tuple

String and array variable assignments can be intermixed. The assignment
parser will implicitly determine the type of value to assign based on the expression.

```
$ let FOO BAR BAZ = [FOO BAR BAZ] BUZZ [BAZ BAR FOO]
$ echo @FOO
FOO BAR BAZ
$ echo $BAR
BUZZ
$ echo @BAZ
BAZ BAR FOO
```

## Type-Checking
[type-checking]: #type-checking

Types may be defined using a colon after each variable name, followed by the name of the
expected type that should be collected.

```
let A:int = 5
let B:int[] = [5 2 3 1]
```

## Assignment Operators
[operators]: #operators

The `=` operator is not the only supported assignment operation: various other arithmetic
operators are supported as well. The action performed depends on the type of the variable
defined on the left hand side of the statement.

```
$ let A:int = 5
$ let A += 2
$ echo $A
7
```

### String Operators
[string-operations]: #string-operations

Arithmetic operations are supported, so long as the variable being assigned to, and the value
to assign, are numbers.

- **Add**: `+=`
- **Subtract**: `-=`
- **Divide**: `/=`
- **Integer Divide**: `//=`
- **Multiply**: `*=`
- **Exponent**: `**=`

### Array Operators
[array-operations]: #array-operations

Arithmetic may also be performed on arrays. These operations can be SIMD-accelerated.

- **Add**: `+=`
- **Subtract**: `-=`
- **Divide**: `/=`
- **Integer Divide**: `//=`
- **Multiply**: `*=`
- **Exponent**: `**=`

There are also some array-specific operations:

- **Append** (`++`): Append values to the array
```
$ let ARRAY = [ 1 2 3 ]
$ let ARRAY ++ 4
$ let ARRAY ++ [5 6 7]
$ echo @ARRAY
1 2 3 4 5 6 7
```

- **Append-Head** (`::`): Insert values at the beginning of the array
```
$ let ARRAY = [ 4 5 6 ]
$ let ARRAY :: [ 1 2 3 ]
$ echo @ARRAY
1 2 3 4 5 6
```

- **Difference** (`\\`): Retain values which are different from the array on the right
```
$ let ARRAY = [ 1 2 3 4 5 6 ]
$ let ARRAY \\ [1 3 5]
$ echo @ARRAY
2 4 6
```

## Error Handling
[errors]: #errors

Assignments that fail should return `1`, otherwise `0` if they succeed.

It is an error for a variable to be assigned more than once in an expression.

```
$ let x x = 1 2
ion: key `x` was specified twice in the assignment
```

Too many of either key or value is also an error.

```
$ let x y = 1 2 3
ion: extra values were supplied, and thus ignored. Previous assignment: 'y' = '2'
```

```
$ let x y z = 1 2
ion: extra keys were supplied, and thus ignored. Previous assignment: 'y' = '2'
```

Type-checking should also print errors.

```
let x:int = apple
ion: assignment error: x: expected int
```

## String Concatenation
[string-concat]: #string-concat

Multiple strings may be combined by enclosing them within double quotes.

```
$ let FOO BAR = FOO BAR
$ let CONCAT = "$FOO  $BAR"
$ echo $CONCAT
FOO  BAR
```

## Array Concatenation
[array-concat]: #array-concat

Arrays may naturally be concatenated into larger arrays using a similar syntax.

```
$ let CONCAT = [@FOO $BAR @BAZ]
$ echo @CONCAT
FOO BAR BAZ BUZZ BAZ BAR FOO
```

# Drawbacks
[drawbacks]: #drawbacks

There are no known drawbacks to this feature.

# Alternatives
[alternatives]: #alternatives

POSIX syntax was briefly considered, but quickly dismissed due to the shortcomings of the design.

# Unresolved questions
[unresolved]: #unresolved-questions

1. What should the complete set of assignment operations be?
