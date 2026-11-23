#!/usr/bin/env bash
# Prints the Homebrew formula for a release from its SHA256SUMS, for the tap at
# github.com/GeronimoCastano/homebrew-kestrel:
#
#   tools/homebrew-formula.sh 0.3.1 SHA256SUMS > Formula/kestrel.rb
#
# A third argument replaces the download location, so CI can install a build that is
# not released yet.
set -euo pipefail

version=${1#v}
sums=$2
base=${3:-https://github.com/GeronimoCastano/kestrel/releases/download/v$version}

sum() {
    local line
    line=$(grep -E "[[:space:]]\*?$1\$" "$sums") || { echo "$sums has no $1" >&2; exit 1; }
    echo "${line%% *}"
}

cat <<EOF
class Kestrel < Formula
  desc "Programming language with checked contracts, proofs, and a native backend"
  homepage "https://github.com/GeronimoCastano/kestrel"
  version "$version"
  license "MIT"

  livecheck do
    url :stable
    strategy :github_latest
  end

  on_macos do
    on_arm do
      url "$base/kestrel-aarch64-apple-darwin.tar.gz"
      sha256 "$(sum kestrel-aarch64-apple-darwin.tar.gz)"
    end
    on_intel do
      url "$base/kestrel-x86_64-apple-darwin.tar.gz"
      sha256 "$(sum kestrel-x86_64-apple-darwin.tar.gz)"
    end
  end

  on_linux do
    url "$base/kestrel-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "$(sum kestrel-x86_64-unknown-linux-gnu.tar.gz)"

    # macOS supplies Clang with the Command Line Tools and LAPACK with Accelerate.
    depends_on "llvm"
    depends_on "openblas"
  end

  # Z3 decides the \`smt\` steps in proofs.
  depends_on "z3"

  def install
    if OS.linux?
      # Homebrew's Clang and OpenBLAS are not on the default search paths, so the
      # wrapper names them.
      libexec.install "kestrel"
      (bin/"kestrel").write_env_script libexec/"kestrel",
        KESTREL_CC:     Formula["llvm"].opt_bin/"clang",
        KESTREL_LAPACK: Formula["openblas"].opt_lib/shared_library("libopenblas")
    else
      bin.install "kestrel"
    end

    generate_completions_from_executable(bin/"kestrel", "completions", shells: [:bash, :zsh, :fish])
  end

  test do
    assert_match "kestrel #{version}", shell_output("#{bin}/kestrel --version")
    (testpath/"hello.kes").write 'print("Hello, Kestrel!")'
    assert_equal "Hello, Kestrel!", shell_output("#{bin}/kestrel run #{testpath}/hello.kes").strip
    (testpath/"batteries.kes").write <<~KES
      import std.linalg
      let A := Tensor([2.0, 1.0, 1.0, 3.0], shape = (2, 2))
      print((A @ linalg.inverse(A) - Tensor([1.0, 0.0, 0.0, 1.0], shape = (2, 2))).norm() < 1e-12)
      theorem positive(n : Int) : n > 0 requires h : n >= 1 {smt}
    KES
    assert_equal "true", shell_output("#{bin}/kestrel run #{testpath}/batteries.kes").strip
  end
end
EOF
