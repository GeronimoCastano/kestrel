# Changelog

All notable changes to Kestrel. Versions follow the policy in the README: within
`0.1`, a patch release fixes bugs; a syntax or feature change bumps the minor
number.

## Unreleased

- Release workflow builds compiler tarballs for macOS arm64, macOS x86_64 and
  Linux x86_64, and packs the editor extension on every tag.
- Linux CI job. The suite passes except two DataFrame comparisons.
- Windows build probe. The port is in progress.
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
