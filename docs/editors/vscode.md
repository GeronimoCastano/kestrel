# VS Code

The Kestrel extension gives `.kes` files highlighting, completion, hover
documentation, diagnostics, inlay hints and the live proof state. This page
covers installing it, pointing it at the compiler, and running a file without
leaving the editor.

## Install the extension

**From a release.** Download `kestrel.vsix` from the
[releases page](https://github.com/GeronimoCastano/kestrel/releases), then:

```sh
code --install-extension kestrel.vsix
```

In the editor you can also open the Extensions view, choose the **...** menu,
pick **Install from VSIX...**, and select the file. Reload VS Code afterwards.

**From this repository.** With the checkout built once (`cargo build`):

```sh
./editors/install-vscode-extension.sh
```

**Marketplace.** Search **Kestrel for VS Code** by `kestrel-lang`, or open the
[listing](https://marketplace.visualstudio.com/items?itemName=kestrel-lang.kestrel-language).

## Point it at the compiler

The extension runs `kestrel serve` for diagnostics and the proof state, and
`kestrel check` for one-off checks. It finds the compiler in this order:

1. `kestrel.compilerPath`, when set.
2. A release or debug binary in this checkout, when it is at least as new as the
   source.
3. `kestrel` on `PATH`.

A Homebrew install needs no setting. A checkout usually wants:

```json
{
  "kestrel.compilerPath": "${workspaceFolder}/target/release/kestrel"
}
```

## What you get

| Feature | Notes |
|---|---|
| Highlighting, brackets, indentation | From the bundled TextMate grammar |
| Snippets | `function`, `class`, `trait`, `theorem`, `match` and the rest |
| Completion | Members, standard modules, imports, named arguments |
| Parameter hints | Names in front of arguments, switchable in settings |
| Hover | Signatures, `///` docs, and typeset `$...$` formulas |
| Go to definition | Local and imported declarations |
| Inlay hints | Inferred types after `let`, parameter names at calls |
| Diagnostics | Types, effects, moves, contracts, proofs, as you type |
| Proof state | Goals, hypotheses and the target for the cursor's theorem |
| Quick fixes | Missing overrides, imports and signatures where the checker asks |

## Run a file

### Code Runner

Install the [Code Runner](https://marketplace.visualstudio.com/items?itemName=formulahendry.code-runner)
extension, then add this to your settings:

```json
{
  "code-runner.executorMap": {
    "kestrel": "kestrel run $fullFileName"
  },
  "code-runner.runInTerminal": true
}
```

Press `Ctrl+Alt+N` (`Cmd+Option+N` on macOS) with a `.kes` file focused. Arguments
come from the program, not from Code Runner, so use the task below when a script
needs them.

### A task

Save this as `.vscode/tasks.json` in your project to get **Tasks: Run Task** and
`Cmd+Shift+B`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "kestrel: run this file",
      "type": "shell",
      "command": "kestrel run ${file}",
      "group": { "kind": "build", "isDefault": true },
      "problemMatcher": [],
      "presentation": { "reveal": "always", "panel": "shared" }
    },
    {
      "label": "kestrel: check this file",
      "type": "shell",
      "command": "kestrel check ${file}",
      "problemMatcher": []
    }
  ]
}
```

Pass arguments with `"args": ["--", "one", "two"]` on a task that runs
`kestrel run ${file} -- one two` instead.

### Notebooks

`kestrel notebook` backs a Jupyter kernel. Install it from `editors/jupyter`;
the extension's completions also run in notebooks.

## Settings that matter

| Setting | Default | What it does |
|---|---|---|
| `kestrel.compilerPath` | empty | Compiler to run; empty uses `PATH` |
| `kestrel.checkOnType.enabled` | `true` | Check while you type, not only on save |
| `kestrel.checkOnType.pauseMs` | `60` | Quiet time after the last keystroke |
| `kestrel.useServer` | `true` | Keep one `kestrel serve` per package |
| `kestrel.inlayHints.parameterNames` | `always` | `always`, `cursorLine` or `off` |
| `kestrel.inlayHints.types` | `true` | Inferred types after `let` |
| `kestrel.proofState.enabled` | `true` | Show the proof state view |
| `kestrel.proofState.math` | `true` | Typeset propositions with TeX |
| `kestrel.expandSymbols` | `true` | Turn `\in` into `∈` when you type a space |

## Troubleshooting

**"kestrel: command not found".** The compiler is not on the `PATH` VS Code sees.
Set `kestrel.compilerPath` to an absolute path, then reload the window.

**"Cannot start Clang".** `kestrel build` and `kestrel run` need a C compiler.
The install commands in [Install](../install.md) bring one: on macOS through the
Command Line Tools (`xcode-select --install`), on Linux through Homebrew, and on
Windows inside the Kestrel install itself.

**A `.kes` file opens as plain text.** The extension is installed but not active.
Reload the window, and check Extensions shows Kestrel enabled.

**The proof state stays empty.** Put the cursor inside a `theorem`, in the
statement or inside its `by { ... }` block. Files without a theorem show nothing.

**Diagnostics stop updating.** A long-running server can hold an old standard
library. Run **Kestrel: Refresh Proof State**, or reload the window after
upgrading the compiler.
