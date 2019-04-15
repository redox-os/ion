# Loops

Loops enable repeated execution of statements until certain conditions are met. There are
currently two forms of loop statements: for loops, and while loops.

## For Loops

For loops take an array of elements as the input; looping through each statement in the block
with each element in the array. If the input is a string, however, that string will automatically
coerce into a newline-delimited array.

```sh
for element in @array
    echo $element
end
```

## Splitting Arguments

When working with strings that you would like to splice into multiple elements for iteration, see
the splicing method, `@split`:

```sh
let value = "one two three four"
for element in @split(value)
    echo $element
end
```

By default, this will split a string by whitespace. Custom patterns may also be provided:

```sh
let value = "one,two,three,four"
for element in @split(value ',')
    echo $element
end
```

A convenience method is also provided for `@split(value '\n')`: `@lines`

```sh
let file = $(cat file)
for line in @lines(file)
    echo = $line =
end
```

## Breaking From Loops

Sometimes you may need to exit from the loop before the looping is finished. This is achievable
using the `break` keyword.

```sh
for element in {1..=10}
    echo $element
    if test $element -eq 5
        break
    end
end
```

```
1
2
3
4
```

## Continuing Loops

In other times, if you need to abort further execution of the current loop and skip to the next
loop, the `continue` keyword serves that purpose.

```sh
for elem in {1..=10}
    if test $((elem % 2)) -eq 1
        continue
    end
    echo $elem
end
```

```
2
4
6
8
10
```

## While Loops

While loops are useful when you need to repeat a block of statements endlessly until certain
conditions are met. It works similarly to if statements, as it also executes a command and
compares the exit status before executing each loop.

```sh
let value = 0
while test $value -lt 6
    echo $value
    let value += 1
end
```

```
0
1
2
3
4
5
```

## Chunked Iterations

Chunked iterations allow fetching multiple values at a time.

```sh
for foo bar bazz in {1..=10}
    echo $foo $bar $bazz
end
```

```
1 2 3
4 5 6
7 8 9
10
```
