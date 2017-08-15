# Implicit `cd`

Like the [Friendly Interactive Shell](https://fishshell.com/), Ion also supports
executing the `cd` command automatically
when given a path. Paths are denoted by beginning with `.`/`/`/`~`, or ending with `/`.

```ion
~/Documents # cd ~/Documents
..          # cd ..
.config     # cd .config
examples/   # cd examples/
```
