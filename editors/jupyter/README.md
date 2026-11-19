# Kestrel kernel for Jupyter

This folder holds a Jupyter kernel for Kestrel. It runs `kestrel notebook`, which keeps one
native session alive for the notebook.

## Install

You need a built `kestrel` (`cargo build --release`), Python 3.9 or newer, and `ipykernel`.

```sh
pip install -e editors/jupyter
python3 -m kestrel_kernel.install --user --kestrel "$PWD/target/release/kestrel"
jupyter lab
```

Pick **Kestrel** in the launcher. Without `--kestrel`, the kernel looks for `$KESTREL`, then
`kestrel` on `PATH`, then `target/release/kestrel` or `target/debug/kestrel` above this folder.
Set `KESTREL_LINK="z m"` to link system libraries into every cell.

A running kernel keeps the compiler process and its embedded standard library
from when it started. After rebuilding `kestrel` or changing `std.kesdas`,
restart the kernel; otherwise cells keep seeing the old members.

## Architecture

This Python package is a Jupyter protocol adapter, not a Kestrel interpreter. It receives
cell text over Jupyter's messaging layer and passes it to one long-running
`kestrel notebook` process. That Rust process checks, lowers and compiles each cell with
Clang, then runs the native code in the same session. Python never holds a `Series`,
`DataFrame` or intermediate value; it only relays text, HTML and errors, and serves completion
from JSON metadata. Executed Kestrel code therefore runs at native speed. The Python layer
costs per-cell protocol and compilation overhead, not per-element interpretation.

A pure Rust kernel using ZeroMQ would remove the Python process, but it would not make the
compiled program faster.

## How cells run

- A cell is one entry in a session. Names, functions and types stay defined for later cells.
- The value of the last expression in a cell shows. Earlier expressions run and print nothing.
- If a cell does not compile, its statements run one at a time. The ones before the bad
  statement still take effect, and the error shows line numbers counted from the cell.
- `?` works at the top level. An `Error` ends the cell and appears as an error output. The
  session and its values stay alive.
- A runtime stop, such as dividing by zero or an index outside a List, ends the cell and
  appears as an error output. The session stays alive, so the names earlier cells bound still
  work; the names the failed cell bound are forgotten, as with `?`.
- A value whose type has an `html()` method shows as HTML. DataFrame and Series do.
- `readLine()` reads nothing in a notebook.

## Completion and types

Completion is type-aware inside a cell and across cells. After
`import std.kesdas.{Series, DataFrame}`, a value such as

```kestrel
let df := DataFrame([Series([1, 2], name = "v")])
```

offers the DataFrame members after `df.`. A selected column keeps its type
through `?`: `df.col("v")?.` offers Series methods, `df.col("v")?.mean().`
offers Scalar methods, and `df.groupBy("v")?.` offers GroupBy methods. Fields
such as `df.names`, `s.data` and `s.index` carry their own types too. Namespace
imports of every standard module complete their names, so `import std.linalg`
makes `linalg.` offer `inverse`, `svd` and the other functions, and
`Tensor([0, 0])` keeps the static type `Tensor[double, (2,)]`. The
kernel uses the same metadata as the VS Code extension, generated with:

```sh
node tools/export_kesdas_metadata.js
node tools/export_kesnet_metadata.js
node tools/export_builtins_metadata.js
```

Run the first after changing `editors/vscode-kestrel/out/kesdas.js`, the second after
changing the KesNet sources, and the last after changing
`editors/vscode-kestrel/out/builtins.js`. Each script accepts `--check`. The
standard-library table also covers List, Array, String, Option and Tensor methods, so
`df.names.` and `df.toCsv().` complete too.

## KesNet

`import std.kesnet.{Linear, Sequential}` brings in layers, losses and the
`Variable` table. `x.` offers the tensor methods, `model.` offers both the
layer's own methods and inherited `Module` methods such as `parameters` and
`zeroGrad`, and chains keep their type: `model.forward(x).relu().sum()` is a
`Variable`, `.shape()` is an `Array[Int]`, and `.data()` is a
`DynTensor[double]`. `import std.kesnet.optim` and `import std.kesnet.data`
complete their respective names. The table is generated from
`src/kesnet.kes`, `src/kesnet_optim.kes` and `src/kesnet_data.kes`.


## kesdas

`import std.kesdas.{Series, DataFrame}` gives you pandas-style tables. Namespace
imports work too: `import std.kesdas as kd` gives `kd.DataFrame(...)`,
`kd.readCsv(...)` and `kd.DataFrame.empty()`.

## Tests

```sh
KESTREL=../../target/release/kestrel python3 -m unittest discover tests
```

The analyzer tests do not start a kernel and run in well under a second.
