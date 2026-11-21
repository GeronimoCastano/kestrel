# Standard library

The standard library ships inside the compiler, so there is nothing to install
and no version to match. Most modules are imported by name; the core types are
already there when your program starts.

```kestrel
import std.json
import std.linalg as la
import std.path.{fileName, join}
```

Every module has a page: what it holds, one section per function with its
arguments, and an example whose output is the one Kestrel prints.

## Core

| Module | What it holds | Page |
|---|---|---|
| `std.core` | The types every program starts with: `String`, `Array`, `List`, `Option`, `Error`. | [documented](core.md) |
| `std.collections` | `List`, `Array`, `Map`, `Set`, and the functions that build them. | [documented](collections.md) |
| `std.str` | Text: search, split, replace, case, trimming. | [documented](str.md) |
| `std.memory` | Allocation and the pool behind arrays and strings. | [documented](memory.md) |
| `std.func` | Function helpers such as `identity`. | [documented](func.md) |
| `std.algebra` | Traits for algebraic structures: `Group`, `Ring`, order. | [documented](algebra.md) |
| `std.set` | Set operations: union, intersection, difference, subsets. | [documented](set.md) |
| `std.nat` | Natural numbers as structural functions, for proofs by induction. | [documented](nat.md) |
| `std.real` | `Real`, an alias for `double`. | [documented](real.md) |
| `std.test` | Assertions and helpers for `@test` functions. | [documented](test.md) |

## Text and data

| Module | What it holds | Page |
|---|---|---|
| `std.json` | JSON text to a `Json` value and back, following RFC 8259. | [documented](json.md) |
| `std.csv` | Comma-separated text, following RFC 4180, to and from a frame. | [documented](csv.md) |
| `std.toml` | TOML 1.0 configuration text to a `Toml` value. | [documented](toml.md) |
| `std.regex` | Regular expressions: match, search, replace, capture. | [documented](regex.md) |
| `std.encoding` | Base64, hex, CRC-32 and SHA-256 over the UTF-8 bytes of a string. | [documented](encoding.md) |
| `std.kesdas` | Series and DataFrames with a pandas-shaped API: grouping, joins, rolling windows, plots. | [documented](kesdas.md) |
| `std.kesplot` | Figures, axes, styles and the SVG backend. | [documented](kesplot.md) |

## Numbers

| Module | What it holds | Page |
|---|---|---|
| `std.math` | Elementary functions: `exp`, `log`, trigonometry, rounding. | [documented](math.md) |
| `std.linalg` | Matrices, vectors, solves, decompositions. | [documented](linalg.md) |
| `std.stats` | Statistics over tensors. | [documented](stats.md) |
| `std.statistics` | Summaries of a `List` of doubles. | [documented](statistics.md) |
| `std.prob` | Probability distributions and their sampling. | [documented](prob.md) |
| `std.random` | Random numbers and seeded generators. | [documented](random.md) |
| `std.complex` | Double-precision complex numbers. | [documented](complex.md) |
| `std.kesnet` | Autograd, layers, training, convolution. | [documented](kesnet.md) |
| `std.ml` | Classical models: regression, classification, scaling. | [documented](ml.md) |

## System

| Module | What it holds | Page |
|---|---|---|
| `std.fs` | Files and directories: read, write, list, copy, remove. | [documented](fs.md) |
| `std.path` | Paths as strings: split, join, walk. | [documented](path.md) |
| `std.io` | Reading input and writing output. | [documented](io.md) |
| `std.args` | Command-line flags and options. | [documented](args.md) |
| `std.term` | ANSI colours and styles, a progress bar, and logs. | [documented](term.md) |
| `std.time` | Clocks, wall time, monotonic time, sleeping. | [documented](time.md) |
| `std.datetime` | Calendar dates and times in UTC, Unix seconds and ISO 8601. | [documented](datetime.md) |
| `std.process` | Running commands, environment variables, exit status. | [documented](process.md) |
| `std.python` | Embedding Python and moving values across the boundary. | [documented](python.md) |
| `std.c` | Calling C functions and passing structs. | [documented](c.md) |

## How a page is written

Each page has a working example at the top, one section per function with its
signature, a table that explains every argument, and a short example with the
result. [std.path](path.md) is the first one and sets the format.
