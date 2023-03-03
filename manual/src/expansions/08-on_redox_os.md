## On Redox Os 

### File scheme

you can also specify absolute paths via the file scheme. 

The file scheme has the prefix "file:".
A path staring with the prefix "file:" is same as giving an absolute path 
which starts with "/" on a Linux system for example.

The following command on Redox Os

```sh
ls file:home/user/*.txt
```
is same as 
```sh
ls /home/user/*.txt
```

Leading "/" will be ignored after the prefix "file:". 
A path like "file:/something" is the same as "file:something" for ion.

