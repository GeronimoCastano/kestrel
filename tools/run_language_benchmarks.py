#!/usr/bin/env python3
"""Run the general-purpose language benchmarks: Kestrel, Rust and Python.

Every program is the same algorithm in each language and prints the same
result. Workload sizes are equal across languages; there is no partial run.
Python's numeric loop is the one long case, around ten seconds per sample.

    python3 tools/run_language_benchmarks.py --runs 5
    python3 tools/run_language_benchmarks.py --runs 3 --only sorting objects
"""

from __future__ import annotations

import argparse
import json
import platform
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "benchmarks"
OUT = BENCH / "build"
KESTREL = ROOT / "target" / "release" / "kestrel"

# Program name -> languages that implement it. bigint has no Rust twin: Rust's
# standard library has no arbitrary-precision integer type.
PROGRAMS = {
    "numeric": ["kestrel", "rust", "python"],
    "collections": ["kestrel", "rust", "python"],
    "strings": ["kestrel", "rust", "python"],
    "trees": ["kestrel", "rust", "python"],
    "channels": ["kestrel", "rust", "python"],
    "objects": ["kestrel", "rust", "python"],
    "functional": ["kestrel", "rust", "python"],
    "sorting": ["kestrel", "rust", "python"],
    "arrays": ["kestrel", "rust", "python"],
    "matflat": ["kestrel", "rust", "python"],
    "bigint": ["kestrel", "python"],
}


def run(command: list[str], cwd: Path = ROOT) -> subprocess.CompletedProcess:
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr[-4000:]}")
    return result


def build(language: str, name: str) -> tuple[float, list[str]]:
    OUT.mkdir(parents=True, exist_ok=True)
    if language == "kestrel":
        binary = OUT / f"kestrel_{name}"
        start = time.perf_counter()
        run([str(KESTREL), "build", "--root", str(BENCH / "kestrel"), "-o", str(binary), str(BENCH / "kestrel" / f"{name}.kes")])
        return time.perf_counter() - start, [str(binary)]
    if language == "rust":
        binary = OUT / f"rust_{name}"
        start = time.perf_counter()
        run(["rustc", "--edition", "2021", "-C", "opt-level=3", "-C", "codegen-units=1", "-C", "target-cpu=native", "-C", "panic=abort", "-o", str(binary), str(BENCH / "rust" / f"{name}.rs")])
        return time.perf_counter() - start, [str(binary)]
    return 0.0, [sys.executable, str(BENCH / "python" / f"{name}.py")]


def measured(command: list[str]) -> tuple[float, int, str]:
    """Wall time, peak resident bytes, and stdout."""
    start = time.perf_counter()
    result = subprocess.run(["/usr/bin/time", "-l", *command], cwd=ROOT, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr[-4000:]}")
    match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
    if not match:
        raise SystemExit(f"cannot read peak memory for {' '.join(map(str, command))}")
    return elapsed, int(match.group(1)), result.stdout


def version(command: list[str]) -> str:
    result = subprocess.run(command, capture_output=True, text=True)
    return (result.stdout or result.stderr).strip().splitlines()[0]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--only", nargs="*", default=list(PROGRAMS))
    parser.add_argument("--json", default=str(BENCH / "language-results.json"))
    args = parser.parse_args()

    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)
    results = {
        "machine": f"{platform.machine()} {platform.platform()}",
        "versions": {
            "clang": version(["clang", "--version"]),
            "rustc": version(["rustc", "--version"]),
            "python": version([sys.executable, "--version"]),
            "kestrel": version([str(KESTREL)]),
        },
        "runs": args.runs,
        "benchmarks": {},
    }
    for name in args.only:
        languages = PROGRAMS[name]
        rows: dict[str, dict] = {}
        expected: str | None = None
        print(f"\n{name}", flush=True)
        for language in languages:
            compile_seconds, command = build(language, name)
            measured(command)  # warm-up; a first launch pays the system's binary scan
            times, peaks, output = [], [], ""
            for _ in range(args.runs):
                elapsed, peak, output = measured(command)
                times.append(elapsed)
                peaks.append(peak)
            if expected is None:
                expected = output
            elif output.strip() != expected.strip():
                raise SystemExit(f"{language} {name} printed different output:\n{output}\nexpected:\n{expected}")
            rows[language] = {
                "seconds": statistics.median(times),
                "minimum_seconds": min(times),
                "peak_bytes": peaks[times.index(min(times))],
                "compile_seconds": compile_seconds,
            }
            print(
                f"  {language:8} {rows[language]['seconds']:8.3f} s  "
                f"{rows[language]['peak_bytes'] / 2**20:8.1f} MiB  compile {compile_seconds:6.2f} s",
                flush=True,
            )
        results["benchmarks"][name] = rows

    Path(args.json).write_text(json.dumps(results, indent=2) + "\n")
    print(f"\nwrote {args.json}")

    print(f"\n{'program':12} {'kestrel s':>10} {'rust s':>8} {'ratio':>7} {'python s':>9} {'vs rust':>8} {'vs py':>7} {'kestrel MB':>11} {'rust MB':>8} {'py MB':>8}")
    for name, rows in results["benchmarks"].items():
        k = rows["kestrel"]
        r = rows.get("rust")
        p = rows["python"]
        ratio_r = f"{k['seconds'] / r['seconds']:.2f}" if r else "-"
        speed_p = f"{p['seconds'] / k['seconds']:.1f}x" if p else "-"
        speed_r = f"{r['seconds'] / k['seconds']:.2f}x" if r else "-"
        print(
            f"{name:12} {k['seconds']:10.3f} {r['seconds'] if r else 0:8.3f} {ratio_r:>7} "
            f"{p['seconds']:9.3f} {speed_r:>8} {speed_p:>7} "
            f"{k['peak_bytes'] / 2**20:11.1f} {r['peak_bytes'] / 2**20 if r else 0:8.1f} {p['peak_bytes'] / 2**20:8.1f}"
        )


if __name__ == "__main__":
    main()
