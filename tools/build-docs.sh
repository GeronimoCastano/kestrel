#!/usr/bin/env bash
# Assembles the documentation site. Pages that live at the repository root are
# copied into the build directory, so the manual and the changelog stay single
# source.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

rm -rf .site-src
mkdir -p .site-src
cp -R docs/. .site-src/
cp SYNTAX.md .site-src/manual.md
cp MATH.md .site-src/math.md
cp KESNET.md .site-src/kesnet.md
cp CHANGELOG.md .site-src/changelog.md
# The manual links to example programs; ship them so the links work on the site.
cp -R testdata .site-src/

case "${1:-build}" in
  build)
    mkdocs build
    # The source tree is only an intermediate; leaving it in the repository
    # makes `kestrel test` pick up a second copy of the example programs.
    rm -rf .site-src
    ;;
  strict)
    mkdocs build --strict
    rm -rf .site-src
    ;;
  serve) mkdocs serve ;;
  *) echo "usage: $0 [build|strict|serve]" >&2; exit 2 ;;
esac
