# Multi-line Arguments

If a line in your script becomes too long, you may signal to Ion to continue reading the next line
by appending an `\` character at the end of the line. This will ignore newlines.

```sh
command arg arg2 \
    arg3 arg4 \
    arg 5
```
