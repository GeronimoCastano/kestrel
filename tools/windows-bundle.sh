#!/usr/bin/env bash
# Assembles the Windows install: kestrel.exe with the C toolchain, LAPACK and Z3 it uses,
# so a Windows user installs nothing else. Runs in Git Bash on a Windows runner and needs
# `gh`, `7z` and `sha256sum`.
#
#   tools/windows-bundle.sh KESTREL_EXE FOLDER     the full install, as the release ships it
#   tools/windows-bundle.sh - FOLDER               everything except kestrel.exe, for CI
#
# Layout:
#   bin/kestrel.exe, bin/libopenblas.dll
#   toolchain/   llvm-mingw: Clang, lld and the MinGW headers and libraries, x86_64 only
#   python/      CPython 3.13 with headers, import library and standard library
#   z3/          z3.exe with the runtime DLLs it ships with
#   licenses/    the licenses of the bundled parts that do not carry their own
set -euo pipefail

executable=$1
out=$2
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

LLVM_MINGW=llvm-mingw-20261117-ucrt-x86_64
OPENBLAS=OpenBLAS-0.3.34-x64
Z3=z3-5.3.3-x64-win
PYTHON=3.13.17

# Downloads one release asset and checks it against its pinned SHA-256.
fetch() {
    local repo=$1 tag=$2 asset=$3 sum=$4
    gh release download "$tag" -R "$repo" -p "$asset" -D "$work" --clobber
    echo "$sum  $work/$asset" | sha256sum -c - > /dev/null
}
fetch mstorsjo/llvm-mingw 20261117 "$LLVM_MINGW.zip" 4d002e330a0f703625aa5ccbc37e96f8f824413e163bfc7336774aabd2c3a412
fetch OpenMathLib/OpenBLAS v0.3.34 "$OPENBLAS.zip" e9cb6134541f36c27346d5fc5995652f060fba227cebbbabcbda5a5a44d7c76b
fetch Z3Prover/z3 z3-5.3.3 "$Z3.zip" 4e48624a6895b49606b3cf91ca4fbcc069fb2973b9706532170a144baa3fd798

# The python.org NuGet package is a complete CPython: python.exe, the DLL, the
# headers, the import library and the standard library. python.org publishes it for
# embedding, and its license is in tools/LICENSE.txt.
PYTHON_SUM=dce409744b088f94c6e37ba41f8100c384c27e029e843237364c602d30d475e5
curl -fsSL -o "$work/python.nupkg" \
    "https://api.nuget.org/v3-flatcontainer/python/$PYTHON/python.$PYTHON.nupkg"
echo "$PYTHON_SUM  $work/python.nupkg" | sha256sum -c - > /dev/null

mkdir -p "$out/bin" "$out/licenses" "$out/z3"
if [ "$executable" != - ]; then
    cp "$executable" "$out/bin/kestrel.exe"
fi

# Clang for x86_64 Windows. Kestrel compiles C for one target, so the other targets'
# libraries, the debugger, Python, the C++ headers and the editor tools stay out.
7z x -o"$work" "$work/$LLVM_MINGW.zip" > /dev/null
rm -rf "$out/toolchain"
mv "$work/$LLVM_MINGW" "$out/toolchain"
(
    cd "$out/toolchain"
    rm -rf aarch64-w64-mingw32 armv7-w64-mingw32 i686-w64-mingw32 arm64ec-w64-mingw32 \
        python busybox include/c++ lib/clang/*/lib/linux
    rm -f lib/clang/*/lib/windows/*-i386.* lib/clang/*/lib/windows/*-aarch64.* \
        lib/clang/*/lib/windows/*-armv7.* lib/clang/*/lib/windows/*-arm.* \
        lib/clang/*/lib/windows/*-arm64ec.*
    cd bin
    rm -f aarch64-* armv7-* i686-* arm64ec-* *uwp* *lldb* libpython* python* \
        clangd.exe clang-tidy.exe run-clang-tidy clang-format.exe clang-scan-deps* \
        llvm-pdbutil.exe
)

# LAPACK and CBLAS in one DLL with no other dependency.
7z x -o"$work/openblas" "$work/$OPENBLAS.zip" bin/libopenblas.dll > /dev/null
cp "$work/openblas/bin/libopenblas.dll" "$out/bin/"
gh api "repos/OpenMathLib/OpenBLAS/contents/LICENSE?ref=v0.3.34" -q .content | base64 -d \
    > "$out/licenses/OpenBLAS.txt"

# Z3 for `smt` proofs. z3.exe loads the C++ runtime DLLs from its own folder.
# CPython for Python interop, in the layout the compiler looks for beside bin.
7z x -o"$work/python" "$work/python.nupkg" tools/ > /dev/null
rm -rf "$out/python"
mv "$work/python/tools" "$out/python"
rm -rf "$out/python/Lib/test" "$out/python/Lib/idlelib" "$out/python/Lib/turtledemo" \
    "$out/python/Lib/ensurepip"

7z x -o"$work" "$work/$Z3.zip" > /dev/null
cp "$work/$Z3/LICENSE.txt" "$out/z3/"
cp "$work/$Z3/bin/z3.exe" "$work/$Z3/bin/"msvcp140*.dll "$work/$Z3/bin/"vcruntime140*.dll \
    "$work/$Z3/bin/vcomp140.dll" "$out/z3/"

du -sh "$out"/* | sed 's/^/bundle: /'
