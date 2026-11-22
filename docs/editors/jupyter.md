# Jupyter

The `kestrel-kernel` package puts Kestrel in JupyterLab and the classic notebook.
Each cell is one entry in a native session, so names and types stay defined
between cells, and the code that runs is compiled C, not an interpreter.

## Install

```sh
pip install kestrel-kernel
python3 -m kestrel_kernel.install --user
jupyter lab
```

The kernel needs a `kestrel` binary. It looks for `$KESTREL`, then `kestrel` on
`PATH`. To name one at install time:

```sh
python3 -m kestrel_kernel.install --user --kestrel /usr/local/bin/kestrel
```

Pick **Kestrel** in the launcher. The first cell can already import the
standard library:

```kestrel
import std.kesdas.{DataFrame, Series}

let sales := DataFrame([
    Series(["north", "south", "north"], name = "region"),
    Series([120, 80, 95], name = "units"),
])
sales.groupBy("region")?.sum()
```

The last expression of a cell is the one that shows. A value whose type has an
`html()` method displays as a table rather than text; `DataFrame` and `Series`
do.

## What persists

A kernel is one long-running `kestrel notebook` process. Names, functions and
types bound in a cell stay available in later cells. A cell that does not
compile runs its statements one at a time: the ones before the bad statement
take effect, and the error points at the line inside the cell.

A runtime stop, such as an index outside a `List`, ends that cell and appears as
an error output. The session stays alive, and the names the failed cell bound
are forgotten.

## Settings

| Variable | Effect |
|---|---|
| `KESTREL` | The compiler to run. |
| `KESTREL_LINK` | Space-separated system libraries to link into every cell, as in `KESTREL_LINK="z m"`. |
| `KESTREL_PYTHON` | The CPython used by `std.python` cells. |

A running kernel keeps the compiler and its embedded standard library from when
it started. After rebuilding `kestrel` or changing a `std` module, restart the
kernel, or cells keep seeing the old members.

## How it works

The Python package is a Jupyter protocol adapter. It receives cell text, passes
it to `kestrel notebook`, and relays text, HTML and errors back. The Kestrel
program runs natively; the Python layer costs per-cell protocol and compilation
time, not per-element interpretation.

Completion is type-aware inside a cell and across cells. After the import above,
`sales.` offers the `DataFrame` members, and a selected column keeps its type.

## Troubleshooting

Check the kernel spec and the compiler it names:

```sh
jupyter kernelspec list
python3 -m kestrel_kernel.install --help
```

If a cell reports that `kestrel` cannot be found, reinstall the spec with
`--kestrel` pointing at the binary, or set `KESTREL` before starting Jupyter.
