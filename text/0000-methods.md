## Methods
[methods]: #methods

Ion also goes a step further and allows the shell to contain some very specific logic that can
be invoked by name: methods. As the shell is parsing a non-braced variable expansion, if it
discovers the `(` character then it will collect all text that follows until it meets the
corresponding `)`, and then executes a function whose name is equal to the text between
the sigil, up to the `(`. The values within the parenthesis are then used as arguments to the
method.

```
let replaced = $replace(string_var, github.com/redox-os gitlab.redox-os.org/redox-os)
let reversed = @reverse(array)
```

### Method Source
[method-source]: #method-source

The first parameter of a method will define the source to enact the method on. Each method has a
_preferred_ type for that method, which allows the user to ellide the sigil. The source does
not have to be a stored type, however, so it is possible to use a method's output as the
input for the source.

> NOTE: Should we use a comma to delimit the source parameter from the argument paramters?

### Method Arguments
[method-arguments]: #method-arguments

Each following parameter will also be evaluated and split accordingly, and used as the arguments
for the method. It's possible to use the output of process expansions and other methods.
