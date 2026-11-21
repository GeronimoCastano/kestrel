# Standard library

The standard library ships inside the compiler, so there is nothing to install
and no version to match. Most modules are imported by name; the core types are
already there when your program starts.

```kestrel
import std.json
import std.linalg as la
import std.path.{fileName, join}
```

A page per module is being written. The compiler remains the source of truth
until a module has one; this table says which pages exist.

## Core

| Module | What it holds | Page |
|---|---|---|
| `std.core` | The types every program starts with: `String`, `Array`, `List`, `Option`, `Error`. | planned |
| `std.collections` | `List`, `Array`, `Map`, `Set`, and the functions that build them. | planned |
| `std.str` | Text: search, split, replace, case, trimming. | planned |
| `std.memory` | Allocation and the pool behind arrays and strings. | planned |
| `std.func` | Function helpers such as `identity`. | planned |
| `std.algebra` | Traits for algebraic structures: `Group`, `Ring`, order. | planned |
| `std.set` | Set operations: union, intersection, difference, subsets. | planned |
| `std.nat` | Natural numbers as structural functions, for proofs by induction. | planned |
| `std.real` | `Real`, an alias for `double`. | planned |
| `std.test` | Assertions and helpers for `@test` functions. | planned |

## Text and data

| Module | What it holds | Page |
|---|---|---|
| `std.json` | JSON text to a `Json` value and back, following RFC 8259. | planned |
| `std.csv` | Comma-separated text, following RFC 4180, to and from a frame. | planned |
| `std.toml` | TOML 1.0 configuration text to a `Toml` value. | planned |
| `std.regex` | Regular expressions: match, search, replace, capture. | planned |
| `std.encoding` | Base64, hex, CRC-32 and SHA-256 over the UTF-8 bytes of a string. | planned |
| `std.kesdas` | Series and DataFrames with a pandas-shaped API: grouping, joins, rolling windows, plots. | planned |

## Numbers

| Module | What it holds | Page |
|---|---|---|
| `std.math` | Elementary functions: `exp`, `log`, trigonometry, rounding. | planned |
| `std.linalg` | Matrices, vectors, solves, decompositions. | planned |
| `std.stats` | Statistics over tensors. | planned |
| `std.statistics` | Summaries of a `List` of doubles. | planned |
| `std.prob` | Probability distributions and their sampling. | planned |
| `std.random` | Random numbers and seeded generators. | planned |
| `std.complex` | Double-precision complex numbers. | planned |
| `std.kesnet` | Autograd, layers, training, convolution. | planned |
| `std.ml` | Classical models: regression, classification, scaling. | planned |

## System

| Module | What it holds | Page |
|---|---|---|
| `std.fs` | Files and directories: read, write, list, copy, remove. | planned |
| `std.path` | Paths as strings: split, join, walk. | [documented](path.md) |
| `std.io` | Reading input and writing output. | planned |
| `std.args` | Command-line flags and options. | planned |
| `std.term` | ANSI colours and styles, a progress bar, and logs. | planned |
| `std.time` | Clocks, wall time, monotonic time, sleeping. | planned |
| `std.datetime` | Calendar dates and times in UTC, Unix seconds and ISO 8601. | planned |
| `std.process` | Running commands, environment variables, exit status. | planned |
| `std.python` | Embedding Python and moving values across the boundary. | planned |
| `std.c` | Calling C functions and passing structs. | planned |

## How a page is written

Each page has a working example at the top, one section per function with its
signature, a table that explains every argument, and a short example with the
result. [std.path](path.md) is the first one and sets the format.
