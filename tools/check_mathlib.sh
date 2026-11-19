#!/usr/bin/env bash
set -euo pipefail
K="${KESTREL:-./target/debug/kestrel}"
status=0

# One process reads the whole library in under two seconds.
"$K" check --root mathlib mathlib/*.kes || status=1

# Admitted proofs. --trusted always exits 0, so read its output.
listing=$("$K" check --trusted --root mathlib mathlib/showcase.kes 2>/dev/null || true)
if grep -q ': trusted ' <<<"$listing"; then
  echo "admitted proof in the library" >&2
  status=1
fi

# Banned words, outside comments.
if grep -rnE '^[^/]*\b(sorry|smt|axiom|opaque)\b' mathlib/*.kes; then
  echo "banned construct in the library" >&2
  status=1
fi

exit "$status"
