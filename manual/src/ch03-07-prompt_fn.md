# Prompt Function

The prompt may optionally be generated from a function, instead of a string. Take note, however,
that prompts generated from functions aren't as efficient, due to the need to perform a fork and
capture the output of the fork to use as the prompt. To use a function for generating the prompt,
simply create a function whose name is **PROMPT**, and the output of that command will be used as
the prompt. Below is an example:

```
fn PROMPT
    echo -n "${PWD}# "
end
```
