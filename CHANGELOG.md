# Changelog

All notable changes to Kestrel. Versions follow the policy in the README:
within a minor line, a patch release fixes bugs; a syntax or feature change
bumps the minor number.

## 0.3.0 - 2026-11-22

- The Windows installer ships CPython 3.13, so Python interop needs no separate
  install. It appends the bundled folder to `PATH`, and a source build still
  takes a python.org CPython.
- Homebrew installs the bash, zsh and fish completion scripts.
- The compiler reports 0.3.0.

## 0.2.0 - 2026-11-21

- `kestrel fmt` formats `.kes` source in place. It works from the token stream,
  so comments, literals and the written form of `x++`, `x += 1` and grouped
  parameters survive. A second run produces the same bytes.
- The style is four spaces and 100 columns. Long calls, lists, parameters and
  match arms go one item per line with a trailing comma. Binary expressions
  break after the loosest operator first. Imports sort within a run and their
  names sort inside `{...}`.
- `--check` reports files that need formatting and exits 1. `--diff` prints a
  unified diff. `--stdin` formats an unsaved buffer.
- The VS Code extension registers a formatting provider, so
  `editor.formatOnSave` works. It runs the compiler with `fmt --stdin`.
- CI checks every tracked `.kes` file with `kestrel fmt --check`.
- `kestrel completions bash|zsh|fish` prints a shell completion script.
- `kestrel new NAME` scaffolds a package: `kestrel.project`, an entry point, a
  module and a test that passes on the first run. `--lib` writes the module and
  test without an entry point; `--git` starts a repository.
- A Packages page documents the project file, dependencies, tests and the
  scaffold command.
- `kestrel doctor` diagnoses an installation: the compiler's version and
  commit, the platform and install layout, Clang, LAPACK, Z3, `PATH` shadowing,
  the package cache, `kestrel.lock` and the VS Code extension. It prints a fix
  next to each problem, takes `--json`, and exits with 1 when a required check
  fails. Its only write is one temporary file in the package cache.
- `kestrel session` loads project C files on Windows, with the bundled Clang
  and with Clang for MSVC.
- Python interop works on Windows in `kestrel run`, `kestrel build` and
  `kestrel session`. Kestrel embeds the CPython that `KESTREL_PYTHON` names, or
  else the first usable one of `python.exe` on `PATH` and `py -3`.
- The compiler reports version 0.2.0.

## 0.1.3 - 2026-11-21

- Windows is supported. One PowerShell command installs the compiler with its
  own Clang, OpenBLAS and Z3, so Visual Studio is not needed.
- Linux is supported, and CI runs the full suite on macOS, Linux and Windows.
- Homebrew installs a prebuilt binary with Z3; on Linux it also brings Clang
  and OpenBLAS.
- Releases ship macOS and Linux tarballs, a complete Windows zip,
  `install.ps1` and `SHA256SUMS`.
- `BCEWithLogitsLoss.backward()` no longer writes past its gradient buffer,
  and every logit now receives its gradient.
- `dyn Iterable` loops over class items no longer depend on C argument order.
- Matrix products are correct without a BLAS library.
- `cbrt` prints the same digits on every platform, and NaN prints as `nan`.
- A leading byte-order mark in a file or in piped input is ignored.
- C `long` is 32 bits on Windows, and Clang's built-in `__size_t` maps to
  `CUInt64`.
- The VS Code extension, version 0.4.5, finds a compiler in the installers'
  default folders when the editor's `PATH` does not name it.
- The complete standard library reference, one page per module under
  `docs/std/`, published as a documentation site.
- Complete member references for the data, plotting and learning libraries
  (`std.kesdas`, `std.kesplot`, `std.kesnet`, `std.ml`), every signature and
  argument included.

## 0.1.2 - 2026-11-20

- The VS Code extension is published as `kestrel-lang.kestrel-language`,
  version 0.4.4, with a rewritten listing page.
- The compiler reports version 0.1.2.

## 0.1.1 - 2026-11-20

- Homebrew tap: `brew install GeronimoCastano/kestrel/kestrel`.
- The extension ships as a VSIX on the releases page.
- `SYNTAX.md` published as the language manual.
- README rewritten with screenshots and a tour.

## 0.1.0 - 2026-11-19

First public release: the compiler, the standard library, the checked
mathematics library, the VS Code extension and the manual.
