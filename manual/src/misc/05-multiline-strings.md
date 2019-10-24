# Multi-line Strings

If a string needs to contain newlines, you may do so by having an open quote, as Ion will only
begin parsing supplied commands that are terminated. Either double or single quotes may be used
for this purpose, depending on which quoting rules that you need.

```sh
echo "This is the first line
    this is the second line
    this is the third line"
```
