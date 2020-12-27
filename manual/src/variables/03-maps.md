# Maps

Maps, (AKA dictionaries), provide key-value data association. Ion has two variants of maps: Hash and BTree. Hash maps are fast but store data in a random order. BTree maps are slower, but keep their data in a sorted order. If not sure what to use, go with Hash maps.

Creating maps uses the same right-hand-side array syntax. However for design simplicity, users must annotate the type to translate the array into a map.

Please note, the map's inner type specifies the value's type and not of the key. Keys will always be typed `str`.

## HashMap
```sh
{{#include ../../../tests/map_vars.ion:hashmap}}
```
```txt
{{#include ../../../tests/map_vars.out:hashmap}}
```

## BTreeMap
```sh
{{#include ../../../tests/map_vars.ion:btreemap}}
```
```txt
{{#include ../../../tests/map_vars.out:btreemap}}
```
