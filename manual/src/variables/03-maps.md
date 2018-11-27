# Maps

Maps, also known as dictionaries, provide key-value association of data. There are two variants of maps within Ion: BTree and Hash. Hash maps store data in a random order, but are fast; whereas BTree maps keep their data in a sorted order, and are slower.

## Create a HashMap

```
let hashmap:hmap[] = [ foo=hello bar=world fizz=I buzz=was bazz=here ]
```

## Create a BTreeMap

```
let hashmap:hmap[] = [ foo=hello bar=world fizz=I buzz=was bazz=here ]
```

## Fetch a variables by key

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