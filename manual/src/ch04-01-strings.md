# String Variables

Using the `let` builtin, a string can easily be created by specifying the name, and an expression
that will be evaluated before assigning it to that variable.


```ion
let git_branch = $(git rev-parse --abbrev-ref HEAD ^> /dev/null)
```

To call a string variable, you may utilize the **$** sigil along with the name of the variable. For more information on expansions, see the expansions section of this manual.

```ion
echo $git_branch
```

## Dropping String Variables

The `drop` command may be used to drop string variables.

```ion
let variable = "testing"
echo $variable
drop variable
echo $variable
```
