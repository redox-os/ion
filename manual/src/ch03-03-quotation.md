# Quoting Rules

In general, double quotes allow expansions within quoted text, whereas single quotes do not.
An exception to the rule is brace expansions, where double quotes are not allowed. When
arguments are parsed, the general rule is the replace newlines with spaces. When double-quoted
expansions will retain their newlines. Quoting rules are reversed for heredocs and for loops.
