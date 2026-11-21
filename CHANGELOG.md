# Changelog

All notable changes to Kestrel. Versions follow the policy in the README: within
`0.1`, a patch release fixes bugs; a syntax or feature change bumps the minor
number.

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
