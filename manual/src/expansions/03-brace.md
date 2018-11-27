# Brace Expansions

Sometimes you may want to generate permutations of strings, which is typically used to shorten
the amount of characters you need to type when specifying multiple arguments. This can be achieved
through the use of braces, where braced tokens are comma-delimited and used as infixes. Any
non-whitespace characters connected to brace expansions will also be included within the brace
permutations.

**NOTE:** Brace expansions will not work within double quotes.

```sh
$ echo filename.{ext1,ext2}
> filename.ext1 filename.ext2
```

Multiple brace tokens may occur within a braced collection, where each token expands the
possible permutation variants.

```sh
$ echo job_{01,02}.{ext1,ext2}
> job_01.ext1 job_01.ext2 job_02.ext1 job_02.ext2
```

Brace tokens may even contain brace tokens of their own, as each brace element will also be
expanded.

```sh
$ echo job_{01_{out,err},02_{out,err}}.txt
> job_01_out.txt job_01_err.txt job_02_out.txt job_02_err.txt
```

Braces elements may also be designated as ranges, which may be either inclusive or exclusive,
descending or ascending, numbers or latin alphabet characters.

```sh
$ echo {1..10}
> 1 2 3 4 5 6 7 8 9

$ echo {1...10}
> 1 2 3 4 5 6 7 8 9 10

$ echo {10..1}
> 10 9 8 7 6 5 4 3 2

$ echo {10...1}
> 10 9 8 7 6 5 4 3 2 1

$ echo {a..d}
> a b c

$ echo {a...d}
> a b c d

$ echo {d..a}
> d c b

$ echo {d...a}
> d c b a
```

It's also important to note that, as brace expansions return arrays, they may be used in for loops.

```ion
for num in {1..10}
    echo $num
end
```
