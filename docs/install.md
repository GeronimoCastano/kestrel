# Install

Kestrel is one binary. It needs no runtime and no package manager; the standard
library is inside it.

The current release is **0.1.2**. Supported: macOS on Apple silicon and Intel,
Linux on x86_64, and Windows 10 1903 or newer on x86_64. CI runs the full test
suite on all three. Windows support arrives in the first release after 0.1.2.

## Homebrew

```sh
brew install GeronimoCastano/kestrel/kestrel
```

Homebrew builds from source at the latest tag, so it needs Rust, which it
installs for the build. The result is on `PATH` as `kestrel`.

## Release archive

Every release has an archive per platform:

- `kestrel-<version>-aarch64-apple-darwin.tar.gz`
- `kestrel-<version>-x86_64-apple-darwin.tar.gz`
- `kestrel-<version>-x86_64-unknown-linux-gnu.tar.gz`
- `kestrel-<version>-x86_64-pc-windows-msvc.zip`

```sh
tar -xzf kestrel-0.1.2-aarch64-apple-darwin.tar.gz
sudo install kestrel-0.1.2-aarch64-apple-darwin/kestrel /usr/local/bin/
kestrel --version
```

## From source

You need Rust 1.95 or newer and Clang on `PATH`.

```sh
git clone https://github.com/GeronimoCastano/kestrel
cd kestrel
cargo build --release
sudo install target/release/kestrel /usr/local/bin/
```

A debug build works too, but its first standard-library check is several times
slower.

## Windows

Kestrel on Windows compiles through Clang and the MSVC linker. Install LLVM, and
the Visual Studio Build Tools for the linker and the Windows SDK:

```powershell
winget install LLVM.LLVM
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Unpack the release zip and add the folder holding `kestrel.exe` to `PATH`, or
build from source as above. `kestrel build -o hello hello.kes` writes
`hello.exe`.

Python interop and project C files inside `kestrel session` are not available on
Windows yet; both stop with an error that says so. `kestrel run` and
`kestrel build` compile project C files as on the other platforms.

## Editor

The VS Code extension is on the
[Marketplace](https://marketplace.visualstudio.com/items?itemName=kestrel-lang.kestrel-language)
as **Kestrel for VS Code**, published by `kestrel-lang`. Search for it in the
Extensions view, or install the VSIX from the
[releases page](https://github.com/GeronimoCastano/kestrel/releases):

```sh
code --install-extension kestrel-vscode-0.4.4.vsix
```

The [editor guide](editors/vscode.md) covers the compiler path, Code Runner and
the settings.

## Requirements

| Tool | Needed for | Notes |
|---|---|---|
| Clang | `kestrel build` and `kestrel run` | Xcode Command Line Tools on macOS, LLVM and the Build Tools on Windows |
| LAPACK or OpenBLAS | `std.linalg` | Loaded at run time when a program uses it; `libopenblas.dll` on `PATH` on Windows |
| Z3 | `smt` proofs | Optional; proofs that need it report a clear error |
| Python 3 | `std.python` | Optional, only for programs that import it |

`kestrel check` needs none of these. It only reads your files.

## Check the install

```sh
kestrel --version
```

```text
kestrel 0.1.2 (compiler and native session)
```

Then compile something:

```kestrel
print("Hello, Kestrel!")
```

```sh
kestrel run hello.kes
```
