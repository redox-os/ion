# Loops

Loops enable repeated execution of statements until certain conditions are met. There are
currently two forms of loop statements: for loops, and while loops.

## For Loops

For loops take an array of elements as the input; looping through each statement in the block
with each element in the array. If the input is a string, however, that string will automatically
coerce into a newline-delimited array.

```ion
for element in @array
    echo $element
end
```

## Breaking From Loops

Sometimes you may need to exit from the loop before the looping is finished. This is achievable
using the `break` keyword.

```ion
for element in {1...10}
    echo $element
    if test $element -eq 5
        break
    end
end
```

## Continuing Loops

In other times, if you need to abort further execution of the current loop and skip to the next
loop, the `continue` keyword serves that purpose.

```ion
for elem in {1...10}
    if test $((elem % 2)) -eq 1
        continue
    end
    echo $elem
end
```

## While Loops

While loops are useful when you need to repeat a block of statements endlessly until certain
conditions are met. It works similarly to if statements, as it also executes a command and
compares the exit status before executing each loop.

```ion
let value = 0
while test $value -lt 6
    echo $value
    let value += 1
end
```
