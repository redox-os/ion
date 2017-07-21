# Expansions

Expansions provide dynamic string generation capabilities. These work identical to the standard
POSIX way, but there are a few major differences: arrays are denoted with an **@** sigil, and have
their own variant of process expansions (**@()**) which splits outputs by whitespace; the
arithmetic logic is more feature-complete, supports floating-point math, and handles larger
numbers; and Ion supports methods in the same manner as the [Oil shell](http://www.oilshell.org/).

- [Variable Expansions](./variable.md)
- [Process Expansions](./process.md)
- [Brace Expansions](./brace.md)
- [Arithmetic Expansions](./arithmetic.md)
- [Method Expansions](./methods.md)
