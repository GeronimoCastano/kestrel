# Packages

A folder with a `kestrel.project` file is a package. `kestrel new` writes one
that checks, runs and tests on the first try:

```sh
kestrel new hello
cd hello
kestrel run main.kes
```

```text
Hello, world!
```

```text
hello/
  kestrel.project
  main.kes
  greet.kes
  tests/
    greet.kes
```

Kestrel finds modules by path. `greet.kes` is module `greet` inside this
package, and `main.kes` imports it with `import greet.{greeting}`. Another
package that declares `depends hello` reaches the same file as `hello.greet`.

`main.kes` holds the program. `greet.kes` holds declarations, because a file
with top-level statements cannot be imported (`E_IMPORT_SCRIPT`), and
`tests/greet.kes` imports the module to test it.

## The project file

The file has two kinds of line: the package name and its dependencies.

```text
package shop
depends matrix
depends geometry from "https://example.org/geometry.git" at "v1.2.0"
```

`package` comes first and names the package. `depends` finds a sibling folder:
`depends matrix` reads `../matrix/kestrel.project`, and that file must say
`package matrix`. A Git dependency adds a URL and a tag or commit.
`kestrel fetch` pins the commit and its tree hash in `kestrel.lock`, and
`kestrel update matrix` moves one pin.

The folder and the package name have to match. Kestrel finds a dependency by
its folder name, so rename both together.

## Tests

`kestrel test` runs every `@test` function under `tests/`, each in its own
process, several at a time.

A test file mirrors the folder it tests. `tests/shop/prices.kes` tests
`shop/prices.kes`: it imports the module as `shop.prices` and can use the
`package` declarations there. A test at the top of `tests/`, like the generated
`tests/greet.kes`, can use the package declarations of files at the package
root.

```kestrel
import greet.{greeting}
import std.test.{assertEq}

@test
function greetsTheWorld() : void {
    assertEq(greeting("world"), "Hello, world!")
}
```

A `@test` outside `tests/` fails the run with `E_TEST_LOCATION`. The function
takes no parameters and returns `void` or `void or Error`; anything else is
`E_TEST_SIGNATURE`.

## Templates

`kestrel new` writes a runnable program by default. `--lib` writes a library
package instead: the manifest, the module and the test, with no `main.kes`.

```sh
kestrel new --lib geometry
cd geometry
kestrel test
```

`--bin` names the default. `--git` starts a Git repository in the new folder,
and every package gets a `.gitignore` for the executable that `kestrel build`
writes beside its source.

## Names

```text
kestrel new NAME [--lib | --bin] [--git]
```

A name starts with a lowercase letter and continues with lowercase letters,
digits or underscores: `hello`, `shop2`, `matrix_ops`. It holds at most 64
characters and is not a Kestrel keyword, so `new` and `import` cannot name a
package. `kestrel new` refuses Windows device names such as `con` and `nul` on
every platform, so a package keeps its name when it moves between machines.

`kestrel new` refuses an existing folder and changes nothing in it. If a later
step fails, it removes the folder it started.
