# Command history

The `history` builtin command can be used to display the command history:
- to display the entire command history, type `history` ;
- if you're only interested in the last N entries, type `history | tail -N`.

Its behavior can be changed via various local variables (see [Variables](#Variables) below).

Ion's history file is located by default in `$HOME/.local/share/ion/history`.

Unlike other shells, Ion by default saves repeated commands only once:
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

The following local variables can be used to modify Ion's history behavior:

### HISTFILE

The file into which the history should be saved. At Ion' startup, the history will be read
from this file, and when it exits, the session's history will be appended to this file.

**Default value:** `$HOME/.local/share/ion/history`

### HISTFILE_ENABLED

Whether the history should be read from/written into the file specified by `HISTFILE`.

**Default value:** `1`

A value of `1` means yes, everything else means no.

### HISTFILE_SIZE

The maximum number of lines kept in the history file when flushed from memory.

**Default value:** `100000`

Ideally, this value should have the same value as `HISTORY_SIZE`. **FIXME:** why ?

**(Currently ignored)**

### HISTORY_IGNORE

Which commands should *not* be saved in the history.

**Default value:** `[ no_such_command whitespace duplicates ]`

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

Specifying an empty array, means that all commands will be saved.

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

**Tip**

I like to add `regex:#ignore$` to my `HISTORY_IGNORE`.
That way, whenever I want to ignore a command on the fly, I just need to add `#ignore` to the
end of the line.

### HISTORY_SIZE

The maximum number of lines contained in the command history in-memory.

**Default value:** `1000`

Ideally, this value should be the same as `HISTFILE_SIZE`. **FIXME:** why ?

**(Currently ignored)**

### HISTORY_TIMESTAMP

Whether a corresponding timestamp should be recorded along with each command.

The timestamp is indicated with a `#` and is unformatted as the seconds since the unix epoch.

**Default value:** `0`

Possible values are `0` (disabled) and `1` (enabled).
