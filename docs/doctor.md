# Diagnose an installation

`kestrel doctor` checks one installation and prints what it finds. It reads files and runs the tools on fixed inputs.

```sh
kestrel doctor
```

```text
kestrel doctor 0.2.0 on macOS (aarch64-apple-darwin)

ok    commit      4f2a9c1d7e30
ok    layout      Homebrew install at /opt/homebrew
ok    clang       Apple clang version 17.0.0 at /usr/bin/clang
ok    lapack      Accelerate framework
warn  z3          not found, so `smt` proofs will fail
      fix: install it with `brew install z3`, or set KESTREL_Z3
ok    path        /opt/homebrew/bin/kestrel is this binary
ok    cache       ~/Library/Caches/kestrel/packages is writable
ok    lock        no kestrel.project in ~/code/shop
ok    vscode      extension 0.4.5 installed

all required checks passed, 1 warning.
```

Each line names a check and its result. A `fix:` line names the command that changes the result.

## Checks

| Check | Level | What it looks at |
|---|---|---|
| `commit` | warning | The build hash. `unknown` means the binary was built outside a Git checkout. |
| `layout` | warning | The install shape: the Windows bundle, Homebrew, a source build, or a standalone binary. |
| `clang` | required | `KESTREL_CC`, then the Clang shipped beside the binary, then `clang` on `PATH`. Doctor runs the candidate on a small C file. |
| `lapack` | warning | `KESTREL_LAPACK`, then Accelerate, then the OpenBLAS and BLAS names `std.linalg` tries. Doctor loads the library and looks for `cblas_dgemm`. |
| `z3` | warning | `KESTREL_Z3`, then the Z3 shipped beside the binary, then `z3` on `PATH`. Doctor sends one query and checks the answer. |
| `path` | warning | The first `kestrel` on `PATH`, compared with the binary you ran. |
| `cache` | warning | The package cache. Doctor writes one file there and removes it. |
| `lock` | required | `kestrel.lock` in the project: valid JSON, a pin for each Git dependency, and a cached source for each pin. |
| `vscode` | warning | The Kestrel extension in the VS Code extension folders. |

A failed required check makes the exit status 1. A warning leaves it at 0.

`lapack` warns instead of failing because `std.linalg` falls back to plain loops when no BLAS library loads. The result is correct and slower. `z3` warns because only `smt` proofs use it.

A malformed `kestrel.lock` fails the check when the project declares dependencies and warns otherwise. A dependency without a pin, a pin with a changed Git source, or a pin without a cached copy warns, and `kestrel fetch` resolves it.

## Options

| Option | What it does |
|---|---|
| `--root DIRECTORY` | Reads `kestrel.project` and `kestrel.lock` from this folder. The default is the current folder. |
| `--json` | Prints one JSON object instead of the table. |

The JSON object carries the version, commit, platform, target, layout, install root, project and cache. Its `checks` array gives each check a `name`, `level`, `status`, `detail` and `fix`. The object ends with the warning count, the failure count and `healthy`. The example below keeps one check and drops the rest.

```json
{"version":"0.2.0","commit":"4f2a9c1d7e30","platform":"macOS","target":"aarch64-apple-darwin","layout":"homebrew","installRoot":"/opt/homebrew","project":"/Users/ana/code/shop","cache":"/Users/ana/Library/Caches/kestrel/packages","checks":[{"name":"z3","level":"warning","status":"warn","detail":"not found, so `smt` proofs will fail","fix":"install it with `brew install z3`, or set KESTREL_Z3"}],"warnings":1,"failures":0,"healthy":true}
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Every required check passed. Warnings may remain. |
| 1 | A required check failed. |
| 2 | The command line was wrong: an unknown option or an extra file. |

A warning never changes the exit status. Read the `warn` lines, or use `--json` and check `"warnings"`.

## What it runs

Doctor probes each tool it finds: `clang --version`, `z3 --version`, one C syntax check through Clang, one `check-sat` query through Z3, and the system library loader for LAPACK. It writes one temporary file inside the package cache to prove the folder is writable, then removes the file and any folder it created.

It makes no network request and installs nothing. It does not check your programs; `kestrel check` does that.

## Privacy

The table shortens your home folder to `~` and `%LOCALAPPDATA%`. The JSON keeps absolute paths.

Doctor reads `KESTREL_CC`, `KESTREL_LAPACK` and `KESTREL_Z3` and prints a value only when it is set. It searches `PATH` and prints the matching binary, not the list of folders.
