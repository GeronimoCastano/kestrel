# Install

One command installs Kestrel with everything it uses: the compiler, a C compiler
for `kestrel build` and `kestrel run`, LAPACK for `std.linalg`, and Z3 for `smt`
proofs. The standard library is inside the compiler.

The current release is **0.2.0**. CI runs the full test suite on macOS, Linux and
Windows, and installs every release the way this page does.

## macOS and Linux

```sh
brew install GeronimoCastano/kestrel/kestrel
```

[Homebrew](https://brew.sh) installs a prebuilt `kestrel` on `PATH`, with Z3. On
macOS, Clang comes from the Command Line Tools that Homebrew itself installs, and
LAPACK from Accelerate. On Linux, Homebrew also brings Clang and OpenBLAS, and the
`kestrel` it installs already knows where they are. `brew upgrade kestrel`
updates it.

## Windows

In PowerShell:

```powershell
irm https://raw.githubusercontent.com/GeronimoCastano/kestrel/main/install.ps1 | iex
```

The installer needs no administrator rights and no Visual Studio. It downloads the
release zip, checks it against the release's `SHA256SUMS`, and unpacks it to
`%LOCALAPPDATA%\Programs\Kestrel`:

| Folder | Holds |
|---|---|
| `bin` | `kestrel.exe` and `libopenblas.dll` |
| `toolchain` | Clang and the Windows headers and libraries, from llvm-mingw |
| `z3` | Z3 for `smt` proofs |

It adds `bin` and, when the archive carries it, `python` to your user `PATH`,
runs a first program to check the install, and
installs the VS Code extension when `code` is on `PATH`. Open a new terminal
afterwards so it sees the new `PATH`. Run the command again to update. To remove
Kestrel, delete the folder and its `PATH` entries.

`KESTREL_VERSION` picks a release instead of the latest, and `KESTREL_HOME` another
install folder:

```powershell
$env:KESTREL_VERSION = '0.2.0'; irm https://raw.githubusercontent.com/GeronimoCastano/kestrel/main/install.ps1 | iex
```

`kestrel run`, `kestrel build` and `kestrel session` compile project C files as
on the other platforms. `kestrel build -o hello hello.kes` writes `hello.exe`.

Python interop embeds CPython 3.13, which the installer ships in `python` beside
`bin`: the interpreter, its DLL, the headers, the import library and the standard
library. `KESTREL_PYTHON` names another `python.exe` when you want it. The
installer appends the bundled folder to your user `PATH`, so a program from
`kestrel build` finds `python313.dll` when it starts.

A source build has no bundled interpreter. It uses `python.exe` on `PATH`,
skipping the Microsoft Store alias, then `py -3`, and takes the first one that has
`include\Python.h` and `libs\python3XY.lib`.

## Release archives

Every [release](https://github.com/GeronimoCastano/kestrel/releases) has the
archives the commands above use, with `SHA256SUMS`:

- `kestrel-aarch64-apple-darwin.tar.gz`
- `kestrel-x86_64-apple-darwin.tar.gz`
- `kestrel-x86_64-unknown-linux-gnu.tar.gz`, for glibc 2.35 or newer
- `kestrel-x86_64-pc-windows-msvc.zip`, the complete Windows install

```sh
tar -xzf kestrel-aarch64-apple-darwin.tar.gz
sudo install kestrel-0.2.0-aarch64-apple-darwin/kestrel /usr/local/bin/
```

An archive on its own needs Clang on `PATH`. On Linux, `std.linalg` also needs
`liblapack` or OpenBLAS from your distribution, and `smt` proofs need `z3`. The
Windows zip carries all three.

## From source

You need Rust 1.95 or newer and Clang on `PATH`.

```sh
git clone https://github.com/GeronimoCastano/kestrel
cd kestrel
cargo build --release
sudo install target/release/kestrel /usr/local/bin/
```

A debug build works too, but its first standard-library check is several times
slower. On Windows, a source build uses Clang for MSVC, which needs LLVM and the
Visual Studio Build Tools; the installer above needs neither. With Clang for
MSVC, a session that loads project C files also runs `llvm-nm`, which LLVM
installs beside `clang`.

## Choosing the tools

Kestrel looks for each tool beside itself first, as the Windows install lays them
out, then on `PATH`. These variables override that:

| Variable | Names |
|---|---|
| `KESTREL_CC` | the C compiler, a Clang |
| `KESTREL_LAPACK` | the LAPACK library to load, such as `libopenblas.so` |
| `KESTREL_Z3` | the Z3 executable |
| `KESTREL_PYTHON` | on Windows, the `python.exe` whose CPython programs embed |

`kestrel check` needs none of these. It only reads your files.

## Editor

The VS Code extension is on the
[Marketplace](https://marketplace.visualstudio.com/items?itemName=kestrel-lang.kestrel-language)
as **Kestrel for VS Code**, published by `kestrel-lang`. The Windows installer
adds it for you when VS Code is on `PATH`. Otherwise search for it in the
Extensions view, or install the VSIX from the release:

```sh
code --install-extension kestrel.vsix
```

The extension finds `kestrel` on `PATH` or in the folders the installers use, so
it works without restarting the editor after an install. The
[editor guide](editors/vscode.md) covers the compiler path, Code Runner and the
settings.

## Shell completions

Homebrew installs completion scripts for bash, zsh and fish. Elsewhere,
`kestrel completions SHELL` prints one.

```sh
# bash, for the current shell only
source <(kestrel completions bash)
```

```zsh
# zsh: keep the file in a folder on fpath
mkdir -p ~/.zsh/completions
kestrel completions zsh > ~/.zsh/completions/_kestrel
```

```fish
kestrel completions fish > ~/.config/fish/completions/kestrel.fish
```

## Check the install

```sh
kestrel --version
```

```text
kestrel 0.2.0 (compiler and native session)
```

Then compile something:

```kestrel
print("Hello, Kestrel!")
```

```sh
kestrel run hello.kes
```

For the whole installation, run `kestrel doctor`. It reports the tools, the
layout, `PATH`, the package cache, `kestrel.lock` and the VS Code extension, and
prints the command that fixes each problem. The [diagnosis page](doctor.md)
documents every check.
