# Key Bindings

There are two pre-set key maps available: Emacs (default) and Vi.
You can switch between them with the `keybindings` built-in command.

```
keybindings vi
keybindings emacs
```

## Vi Mode Prompt Indicators

In the Vi mode, you can define the displayed indicator for normal and insert modes
with the following variables:

```
$ export VI_NORMAL = "[=] "
$ export VI_INSERT = "[+] "
$ keybindings vi
[+] $
```
