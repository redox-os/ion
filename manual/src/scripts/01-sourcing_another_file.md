# Sourcing another file

A ion shell script can also execute another ion shell script and inherit its environment and
variables while doing so.

```sh
source to_source.ion
```

When sourcing another Ion shell file you can also supply it with positional arguments.

```sh
# to_source.ion will now have "first" as the first positional and "second" second positional argument
# Remeber the zeroth postional argument is file name of the executed script. 
# Here "to_source.ion" in this example. 
source to_source.ion "first" "second"
```
