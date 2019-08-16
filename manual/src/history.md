# Command history

## General
- Ions history can be found at **$HOME/.local/share/ion/history**
- The `history` builtin can be used to display the entire command history
  - If you're only interested in the last X entries, use `history | tail -n X`
- The histories\' behavior can be changed via various local variables (see section
  **Variables**)
- Unlike other shells, `ion` saves repeated commands only once:
```sh
# echo "Hello, world!"
Hello, world!
# true
# true
# false
# history
echo "Hello, world!"
true
false
```

## Variables
The following local variables can be used to modify Ions history behavior:

### HISTORY_SIZE
Determines how many entries of the history are kept in memory.

Ideally, this value should be the same as `HISTFILE_SIZE`

**Default value:** `1000`

### HISTORY_IGNORE
Specifies which commands should **NOT** be saved in the history.

This is an array and defaults to an **empty array**, meaning that all commands will be saved.

Each element of the array can take one of the following options:
- `all` <br/>
  All commands are ignored, nothing will be saved in the history.
- `no_such_command` <br/>
  Commands which return `NO_SUCH_COMMAND` will not be saved in the history.
- `whitespace` <br/>
  Commands which start with a [whitespace character](https://doc.rust-lang.org/stable/reference/whitespace.html) will not be saved in the
  history.
- `regex:xxx`  <br/>
  Where xxx is treated as a [regular expression](https://doc.rust-lang.org/regex/regex/index.html).
  Commands which match this regular expression will not be saved in the history.
- `duplicates`  <br/>
  All preceding duplicate commands are removed/ignored from the history after a matching command is entered.

**Default value:** `[ no_such_command whitespace duplicates ]`

**Notes**
- You can specify as many elements as you want.
- Any invalid elements will be silently ignored. They will still be present in the array though.
- You can also specify as many regular expressions as you want (each as a separate element).
- However, note that any command that matches **at least one** element will be ignored.
- (Currently, ) there is no way to specify commands which should always be saved.
- When specifying **regex:**-elements, it is suggested to surround them with single-quotes (`'`)
- As all variables, `HISTORY_IGNORE` is not saved between sessions. It is suggested to set it via
ions init file.
- The `let HISTORY_IGNORE = [ .. ]` command itself is **not effected** except if the assignment
command starts with a whitespace and the **whitespace** element is specified in this assignment.
See the following example:
```sh
# echo @HISTORY_IGNORE

# let HISTORY_IGNORE = [ all ] # saved
# let HISTORY_IGNORE = [ whitespace ] # saved
#  true # ignored
#  let HISTORY_IGNORE = [  ] # saved
#  let HISTORY_IGNORE = [ whitespace ] # ignored
# history
echo @HISTORY_IGNORE
let HISTORY_IGNORE = [ all ] # saved
let HISTORY_IGNORE = [ whitespace ] # saved
 let HISTORY_IGNORE = [  ] # saved
```

**Examples**
```sh
# let HISTORY_IGNORE = [ no_such_command ]
# true # saved
#  true # saved
# false # saved
# trulse # ignored
```

```sh
# let HISTORY_IGNORE = [ 'regex:.*' ] # behaves like 'all'
# true # ignored
#  true # ignored
# false # ignored
# trulse # ignored
```

**Tips**

I like to add `regex:#ignore$` to my `HISTORY_IGNORE`.
That way, whenever I want to ignore a command on the fly, I just need to add `#ignore` to the
end of the line.

### HISTFILE_ENABLED
Specifies whether the history should be read from/written into the file specified by `HISTFILE`.

A value of **1** means yes, everything else means no.

**Default value:** `1`

### HISTFILE
The file into which the history should be saved. At the launch of ion the history will be read
from this file and when ion exits, the history of the session will be appended into the file.

**Default value:** `$HOME/.local/share/ion/history`

### HISTFILE_SIZE
Specifies how many commands should be saved in `HISTFILE` at most.

Ideally, this value should have the same value as `HISTORY_SIZE`.

**Default value:** `100000`

### HISTORY_TIMESTAMP
Specifies whether a corresponding timestamp should be recorded along with each command.

The timestamp is indicated with a `#` and is unformatted as the seconds since the unix epoch.

Possible values are `0` (disabled) and `1` (enabled).

**Default value:** `0`
