# Maps

Maps, (AKA dictionaries), provide key-value data association. Ion has two variants of maps: BTree and Hash. Hash maps are fast but store data in a random order; whereas BTree maps are slower but keep their data in a sorted order. If not sure what to use, it's best to go with Hash maps.

Creating maps uses the same right-hand-side array syntax. However for design simplicity, users must annotate the type to translate the array into a map.

Please note, the map's inner type specifies the value's type and not of the key. Keys will always be typed `str`.

## Create a HashMap

```
let hashmap:hmap[str] = [ foo=hello bar=world fizz=I buzz=was bazz=here ]
```

## Create a BTreeMap

```
let btreemap:bmap[str] = [ foo=hello bar=world fizz=I buzz=was bazz=here ]
```

## Fetch a variable by key

```
let x = bazz
echo @hashmap[bar] @hashmap[$x]
```

## Insert a new key

```
let x[bork] = oops
```

## Iterate keys in the map

```
echo @keys(hashmap)
```

## Iterate values in the map

```
echo @values(hashmap)
```

## Iterate key/value pairs in the map

```
echo @hashmap
```

## Iterate key/value pairs in a loop

```
for key value in @hashmap
    echo $key: $value
end
```
