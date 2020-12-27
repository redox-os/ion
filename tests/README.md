## Changing tests

This tests are also used to generate the manual.
Please make sure to update the manual, if you change tests.

## Examples

The files in this directory are simple example scripts that are used to test
the state of the shell as it is developed. When the **run_examples.sh** script
is executed, it will build Ion and execute each of the ion scripts here, and
compare their outputs to their assoicated **out** files.

```
TOOLCHAIN=stable ./run_examples.sh
```

For more elaborate examples of Ion usage, check out the **advanced** directory.
