# Matches

Matches will evaluate each case branch, and execute the first branch which succeeds.
A case which is `_` will execute if all other cases have failed.

```sh
match $string
    case "this"
        echo "do that"
    case "that"
        echo "else this"
    case _; echo "not found"
end
```

## Matching string input with array cases

If the input is a string, and a case is an array, then a match will succeed if at
least one item in the array is a match.

```sh
match five
    case [ one two three ]; echo "one of these matched"
    case [ four five six ]; echo "or one of these matched"
    case _; echo "no match found"
end
```

## Matching array input with string cases

The opposite is true when the input is an array, and a case is a string.

```sh
match [ five foo bar ]
    case "one"; echo "this"
    case "two"; echo "that"
    case "five"; echo "found five"
    case _; echo "no match found"
end
```

## Match guards

Match guards can be added to a match to employ an additional test

```sh
let foo = bar
match $string
    case _; echo "no match found"
    case "this" if eq $foo bar
        echo "this and foo = bar"
    case "this"
        echo "this and foo != bar"
end
```
