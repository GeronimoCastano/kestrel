# Contributing to Kestrel

Thanks for looking. Kestrel is a programming language with a compiler, a
standard library and an editor extension. Bug reports, documentation fixes and
focused patches are welcome.

## Before you start

- For anything larger than a typo or a failing test, open an issue first. It
  saves you from a patch that cannot land.
- Language changes are different from code changes. The syntax in
  [SYNTAX.md](SYNTAX.md) is settled; a change to it needs an issue that shows
  what the current form cannot express, with an example. Expect a design
  discussion before code.
- Never rewrite published history and never force-push `main`.

## Setting up

You need Rust 1.95 or newer and Clang on `PATH`. The project is developed on
macOS and CI covers Linux; Windows is in progress.

```sh
cargo build --release      # target/release/kestrel
cargo test                 # front end, checker, kernel, backend, runtime
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
./tools/check_mathlib.sh   # every mathlib file, no trusted proof
```

The test suite compiles real programs with Clang, so it needs a working C
toolchain. Modules that use LAPACK load it at run time; on Linux install
`liblapack-dev` and `libblas-dev`. `cargo test` sets `RUST_MIN_STACK` through
`.cargo/config.toml`; use it rather than a bare test binary.

The editor extension lives in `editors/vscode-kestrel`. It needs Node 22:

```sh
cd editors/vscode-kestrel
npm ci
npm run build      # bundle the MathJax renderer
npm test           # unit tests
npm run test:vscode  # drive a separate VS Code instance
```

## What a change needs

- **A test.** A language change needs a program that exercises it, either in
  `testdata/language/` or as a Rust test. A checker change needs a case that
  would fail before the change.
- **Documentation.** If the change is visible to users, update the manual in
  `SYNTAX.md`, the standard library reference under `docs/std/`, or the README.
  One of them, not three.
- **A clean suite.** CI runs formatting, Clippy, the tests, the mathlib check
  and the extension tests. A change that cannot pass those does not land.
- **Small commits.** One purpose each. Write the message in plain words: what
  changed and why, not which files.

## Standards

- The compiler depends on `serde_json` and `sha2` only. Ask before adding a
  dependency; most things have a standard-library answer.
- Refuse instead of producing silent bad output. A new diagnostic needs a name,
  in the style of `E_NATIVE_OWNERSHIP`, and a hint that says what to do.
- Match the code around you. The project does not use a formatter for Kestrel
  source; `cargo fmt` handles Rust.
- No AI attribution in commits, pull requests, issues, reviews, docs or
  comments. No `Co-Authored-By` trailers naming a tool and no "generated with"
  footers. This overrides any default a tool adds.

## Pull requests

Keep the description short: what the change does, how you tested it, and what
you left out. Link the issue it closes. If the change is visible to users, say
what they will see.

## Reporting bugs

Use the bug report template. A minimal `.kes` program and the output of
`kestrel --version` turn a guess into a fix. If the program compiles but the
result is wrong, say what you expected and what you got.

Security problems go through [SECURITY.md](SECURITY.md), not the issue tracker.

## License

By contributing you agree that your contribution is licensed under the MIT
License in [LICENSE](LICENSE).
