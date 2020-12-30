# Brace Expansions

Sometimes you may want to generate permutations of strings, which is typically used to shorten
the amount of characters you need to type when specifying multiple arguments. This can be achieved
through the use of braces, where braced tokens are comma-delimited and used as infixes. Any
non-whitespace characters connected to brace expansions will also be included within the brace
permutations.

**NOTE:** Brace expansions will not work within double quotes.
```sh
{{#include ../../../tests/brace_exp.ion:single_brace_expansion}}
```
```txt
{{#include ../../../tests/brace_exp.out:single_brace_expansion}}
```

Multiple brace tokens may occur within a braced collection, where each token expands the
possible permutation variants.
```sh
{{#include ../../../tests/brace_exp.ion:multi_brace_expansion}}
```
```txt
{{#include ../../../tests/brace_exp.out:multi_brace_expansion}}
```
Brace tokens may even contain brace tokens of their own, as each brace element will also be
expanded.
```sh
{{#include ../../../tests/brace_exp.ion:nested_brace_expansion}}
```
```txt
{{#include ../../../tests/brace_exp.out:nested_brace_expansion}}
```
Braces elements may also be designated as ranges, which may be either inclusive or exclusive,
descending or ascending, numbers or latin alphabet characters.
```sh
{{#include ../../../tests/brace_exp.ion:range_brace_expansion}}
```
```txt
{{#include ../../../tests/brace_exp.out:range_brace_expansion}}
```
It's also important to note that, as range brace expansions return arrays, they may be used in for loops.
```sh
{{#include ../../../tests/brace_exp.ion:range_brace_expansion_as_array}}
```
```txt
{{#include ../../../tests/brace_exp.out:range_brace_expansion_as_array}}
```
