#!/usr/bin/env python3
"""Runs the benchmarks and fails when Kestrel's time relative to Rust's grew.

Wall-clock timings differ between machines, so the comparison uses the ratio of
Kestrel's median time to Rust's on the same run. A ratio that grows past the
baseline by more than the tolerance is a regression.
"""
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, default=ROOT / "benchmarks/results.json")
    parser.add_argument("--current", type=Path, help="where to keep this run's results")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--tolerance", type=float, default=1.7)
    arguments = parser.parse_args()

    baseline = json.loads(arguments.baseline.read_text())["benchmarks"]
    with tempfile.TemporaryDirectory() as folder:
        current_path = Path(folder) / "results.json"
        subprocess.run(
            [
                sys.executable,
                str(ROOT / "tools/run_benchmarks.py"),
                "--runs",
                str(arguments.runs),
                "--languages",
                "kestrel",
                "rust",
                "--out",
                str(current_path),
            ],
            cwd=ROOT,
            check=True,
        )
        current = json.loads(current_path.read_text())["benchmarks"]
        if arguments.current:
            arguments.current.parent.mkdir(parents=True, exist_ok=True)
            arguments.current.write_text(current_path.read_text())

    failed = False
    print(f"{'benchmark':12} {'kestrel':>9} {'rust':>8} {'ratio':>7} {'baseline':>9}")
    for name, rows in baseline.items():
        if name not in current:
            continue
        kestrel = current[name]["kestrel"]["seconds"]
        rust = current[name]["rust"]["seconds"]
        ratio = kestrel / rust
        old = rows["kestrel"]["seconds"] / rows["rust"]["seconds"]
        flag = ""
        if ratio > old * arguments.tolerance:
            flag = "  REGRESSION"
            failed = True
        print(f"{name:12} {kestrel:9.3f} {rust:8.3f} {ratio:7.2f} {old:9.2f}{flag}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
