# Read-eval-print loop

## Implicit `cd`
Like the [Friendly Interactive Shell](https://fishshell.com/), Ion also supports
executing the `cd` command automatically
when given a path. Paths are denoted by beginning with `.`/`/`/`~`, or ending with `/`.
```sh
~/Documents # cd ~/Documents
..          # cd ..
.config     # cd .config
examples/   # cd examples/
```

## Multi-line Arguments
If a line in your script becomes too long, appending `\` will make Ion ignore newlines
and continue reading the next line.
```sh
command arg arg2 \
    arg3 arg4 \
    arg 5
```

## Multi-line Strings
If a string needs to contain newlines, you use an open quote. Ion will only
begin parsing supplied commands that are terminated. Either double or single quotes can be used.
```sh
echo "This is the first line
    this is the second line
    this is the third line"
```

## Prompt Function
The prompt may optionally be generated from a function, instead of a string. Due to the need to
perform a fork an capture of its output as prompt, prompts generated from functions aren't as
efficient. Below the requirement to use the function with name **PROMPT**:
```sh
fn PROMPT
    echo -n "${PWD}# "
end
```

## Key Bindings
There are two pre-set key maps available: **Emacs (default)** and **Vi**.
You can switch between them with the `keybindings` built-in command.
```sh
keybindings vi
keybindings emacs
```
**Vi keybinding**: You can define the displayed indicator for normal and insert modes
with the following variables:
```sh
$ export VI_NORMAL = "[=] "
$ export VI_INSERT = "[+] "
$ keybindings vi
[+] $
```
