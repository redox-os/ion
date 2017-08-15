# Tuple Assignments

Ion also supports assigning multiple variables at once, which can increase readability and save
some precious CPU cycles. The general trend is that the less statements that you execute, the
faster your scripts will execute, but there are some exceptions to the rule -- see the general
tips in the miscellanious section. In addition to assigning multiple variables, this can also
be used to swap variables.

```ion
let a b = 1 2
let a b = [1 2]
let a b = [$b $a]
```

Do note, however, that if you supply too many values, they will be ignored.

```ion
$ let a b = 1 2 3
$ echo $a $b
> 1 2
```

