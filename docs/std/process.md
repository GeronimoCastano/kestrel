# std.process

Arguments, environment, the working directory, running a command, and exiting.

```kestrel
import std.process

print(process.cwd())
print(process.env("HOME"))
print(process.args)

let (status, output) := process.run("echo hi")?
print(status)     // 0
print(output)     // hi
```

## The program

### `args : List[String]`

The words after the program name, in order. It is a value, not a call.

```kestrel
for (arg in process.args) {
    print(arg)
}
```

## Environment and location

### `env(name : String) : Option[String]`

The value of an environment variable, or `None` when it is not set.

| Argument | Meaning |
|---|---|
| `name : String` | The variable's name. |

### `cwd() : String`

The current working directory, as a path.

## Running a command

### `run(command : String) : (Int, String) or Error`

Runs the command through the shell and returns its exit status and output.

| Argument | Meaning |
|---|---|
| `command : String` | The command line, exactly as a shell would receive it. |

```kestrel
let (status, output) := process.run("ls src")?
if status == 0 {
    print(output)
}
```

A command that cannot start is an error. A command that starts and fails is not:
the status tells you.

## Leaving

### `exit(code : Int) : void`

Ends the process with an exit code.

| Argument | Meaning |
|---|---|
| `code : Int` | The status the shell sees. |

## Notes

- `run` goes through the shell, so quoting and redirection follow the shell's
  rules.
- Every function here except `args` carries `Effects`, because the outside world
  is involved.
