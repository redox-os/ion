# Script Executions

Scripts can be created by designating Ion as the interpreter in the shebang line.

```sh
#!/usr/bin/env ion
```

Then writing the script as you would write it in the prompt. When finished, you can execute the
shell by providing the path of the script to Ion as the argument, along with any additional
arguments that the script may want to parse. Arguments can be accessed from the **@args** array,
where the first element in the array is the name of the script being executed.

```sh
#!/usr/bin/env ion

if test $len(@args) -eq 1
    echo "Script didn't receive enough arguments"
    exit
end

echo Arguments: @args[1..]
```

## Executing commands at start of interactive ion session

Commands can be executed at the start of an interactive ion session.
This can be useful to set up environmental variables specific to ion,
,set aliases , set vim keybindings, etc.

Ion reads those initial commands from a file called "initrc". 
Ion looks for this file in its configuration folder
according to the [xdg standard](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html).

This typically results in the configuration path ~/.config/ion by default. 
So ion searches for the file initrc in the folder ~/.config/ion normally.

For example the following content of the initrc file 
causes ion to start with vim bindings enabled.

```sh
keybindings vi 
```

You can change the location where ion looks for its configuration folder 
by setting the environmental variable **$XDG_CONFIG_HOME** to a different path.

```sh 
export XDG_CONFIG_HOME='/home/some_user/myconfig' 
```

In this example ion will look at the path /home/some_user/myconfig/ion  
for the initrc file.

