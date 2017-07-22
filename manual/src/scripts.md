# Script Executions

Scripts can be created by designating Ion as the interpreter in the shebang line.

```ion
#!/usr/bin/env ion
```

Then writing the script as you would write it in the prompt. When finished, you can execute the
shell by providing the path of the script to Ion as the argument, along with any additional
arguments that the script may want to parse. Arguments can be accessed from the **@args** array,
where the first element in the array is the name of the script being executed.

```ion
#!/usr/bin/env ion

if test $len(@args) -eq 1
    echo "Script didn't receive enough arguments"
    exit
end

echo Arguments: @args[1..]i
```
