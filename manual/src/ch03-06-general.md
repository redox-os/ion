# General Tips

## Let Arithmetic vs Arithmetic Expansions

Using **let** arithmetic is generally faster than **$(())** expansions. The arithmetic expansions
should be used for increasing readability, or more complex arithmetic; but if speed is important,
multiple let arithmetic statements will tend to be faster than a single arithmetic expansion.
