# Kestrel

A small language with a picky compiler. It checks your types, your errors, your
memory and your math before the program runs, then hands the work to Clang.

```kestrel
enum Shape { Circle(double), Square(double) }

function area(shape : Shape) : double = match shape {
    Circle(r) => 3.14159 * r * r,
    Square(s) => s * s,
}

print(f"{area(Circle(2.0)):.2f}")
```

```text
12.57
```

## Start here

<div class="grid cards" markdown>

-   :material-download:{ .lg .middle } **Install**

    ---

    Homebrew, a release tarball, or a build from source. One binary, no runtime.

    [:octicons-arrow-right-24: Install](install.md)

-   :material-book-open-variant:{ .lg .middle } **Language manual**

    ---

    Every form in the language, from `let` to `by { ... }`, in one page.

    [:octicons-arrow-right-24: Read the manual](manual.md)

-   :material-function-variant:{ .lg .middle } **Standard library**

    ---

    One page per module, written by hand, with an example for every function.

    [:octicons-arrow-right-24: Browse the library](std/index.md)

-   :material-puzzle:{ .lg .middle } **Editor**

    ---

    Highlighting, completion, inlays, a live proof state, and the compiler's
    errors as you type.

    [:octicons-arrow-right-24: Set up VS Code](editors/vscode.md)

</div>

## What it checks

- **Names and shapes.** Types line up, every `match` covers every case, and a
  call passes what the function declared.
- **Outside work.** Printing, reading a file and sending a request need
  `with Effects`. A function without that mark cannot touch the world.
- **Ownership.** A value moves once, views cannot outlive their owner, and a
  borrowed object cannot be handed away.
- **Proofs.** A theorem is checked by the kernel, and a function can make its
  contract depend on one.

## Proofs are part of the language

```kestrel
theorem subsetRefl[U](A : Set[U]) : A ⊆ A by {
    intro x
    intro hx
    exact hx
}
```

The proof kernel runs inside the compiler. Code generation erases proofs, so a
checked theorem costs nothing at run time.

## A native program

`kestrel build` emits C11 and asks Clang for an executable. There is no
interpreter and no virtual machine; a Kestrel program is a normal native program.

```sh
kestrel run hello.kes
```

```text
Hello, Kestrel!
```

## Status

Kestrel is version 0.1. It runs on macOS, Linux and Windows, and CI tests all
three on every pull request. [Install](install.md) has the details.
