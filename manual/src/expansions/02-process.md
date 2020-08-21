# Process Expansions

Ion supports two forms of process expansions: string-based process expansions (**$()**) that are
commonly found in POSIX shells, and array-based process expansions (**@()**), a concept borrowed
from the Oil shell. Where a string-based process expansion will execute a command and return a
string of that command's standard output, an array-based process expansion will split the output
into an array delimited by whitespaces.

```sh
let string = $(cmd args...)
let array = [ @(cmd args...) ]
```
**NOTES:**
- To split outputs by line, see [@lines($(cmd))](https://doc.redox-os.org/ion-manual/html/expansions/05-method.html#lines).
- `@(cmd)` is equivalent to [@split($(cmd))](https://doc.redox-os.org/ion-manual/html/expansions/05-method.html#split).
