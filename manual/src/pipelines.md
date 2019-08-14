# Pipelines

## Redirection

Redirection will write the output of a command to a file.

### Redirect Stdout

```sh
command > stdout
```

### Redirect Stderr

```sh
command ^> stderr
```

### Redirect Both

```sh
command &> combined
```

### Multiple Redirection

```sh
command > stdout ^> stderr &> combined
```

### Concatenating Redirect

Instead of truncating and writing a new file with `>`, the file can be appended to with `>>`.

```sh
command > stdout
command >> stdout
```

## Pipe

### Pipe Stdout

```sh
command | command
```

### Pipe Stderr

```sh
command ^| command
```

### Pipe Both

```sh
command &| command
```

## Combined

```sh
command | command > stdout
```

## Detaching processes

### Send to background

```sh
command &
```

### Disown (detach from shell)

```sh
command &!
```
