# Functions

Functions help scripts to reduce the amount of code duplication and increase readability. Ion supports the creation of functions with a similar syntax to other languages.

The basic syntax of functions is as follows:

```sh
fn square
    let x = "5"
    echo $(( x * x ))
end

square
square
```

Every statement between the `fn` and the `end` keyword is part of the function. On every function call, those statements get executed.  That script would ouput "25" two times.

If you want the square of something that isn't five, you can add arguments to the function.

```sh
fn square x
    echo $(( x * x ))
end

square 3
```

## Type checking

Optionally, you can add type hints into the arguments to make ion check the types of the arguments:

```sh
fn square x:int
    echo $(( x * x ))
end

square 3
square a
```

You'd get as output of that script:

```
9
ion: function argument has invalid type: expected int, found value 'a'
```

You can use any of the [supported types](ch04-00-variables.md#Supported Types).

## Multiple arguments

As another example:

```
fn hello name age:int hobbies:[str]
    echo "$name ($age) has the following hobbies:""
    for hobby in @hobbies
        echo "  $hobby"
    end
end

hello John 25 [ coding eating sleeping ]
```

## Function piping


As with any other statement, you can pipe functions using `read`.

```sh
fn format_with pat
    read input
    echo $join(@split(input), $pat)
end

echo one two three four five | format_with "-"
```
