#!/usr/bin/env python3
"""Measure per-operation ML time by differencing one and many repetitions.

Each workload program takes a repetition count as its first argument. Running
it once and N times and subtracting removes process startup, imports and model
fitting, leaving the loop body. Numbers are wall-clock medians of several runs
on the same machine.
"""
import argparse
import json
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KESTREL = ROOT / "target" / "release" / "kestrel"
WORKLOADS = [
    "kmeans20k", "knnpredict5k", "linearregression20k", "logisticregression20k",
    "metrics100k", "minmaxscaler100k", "pca10k", "polynomialfeatures20k",
    "standardscaler100k", "traintestsplit50k",
]


def run(command, runs):
    times = []
    for _ in range(runs):
        started = time.perf_counter()
        done = subprocess.run(command, capture_output=True, text=True, cwd=ROOT)
        times.append(time.perf_counter() - started)
        if done.returncode != 0:
            raise RuntimeError(f"command failed: {' '.join(command)}\n{done.stderr}")
    return statistics.median(times)


def slope(command, repetitions, runs):
    one = run(command + ["1"], runs)
    many = run(command + [str(repetitions)], runs)
    return (many - one) / (repetitions - 1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repetitions", type=int, default=20)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--only", nargs="*", default=None)
    parser.add_argument("--json", default=str(ROOT / "benchmarks" / "ml-kernel-results.json"))
    args = parser.parse_args()
    workloads = args.only or WORKLOADS
    build = Path(tempfile.mkdtemp(prefix="kestrel-ml-"))
    results = {}
    print(f"{'workload':22} {'kestrel ms':>10} {'python ms':>10} {'ratio':>7}")
    for name in workloads:
        source = ROOT / "benchmarks" / "libraries" / "ml" / "kestrel" / f"{name}.kes"
        binary = build / name
        done = subprocess.run([str(KESTREL), "build", str(source), "-o", str(binary)],
                              capture_output=True, text=True, cwd=ROOT)
        if done.returncode != 0:
            raise RuntimeError(f"build failed: {name}\n{done.stderr}")
        kestrel = slope([str(binary)], args.repetitions, args.runs)
        python = slope([sys.executable, str(ROOT / "benchmarks" / "libraries" / "ml" / "python" / f"{name}.py")],
                       args.repetitions, args.runs)
        ratio = kestrel / python if python else 0.0
        results[name] = {"kestrel_seconds": kestrel, "python_seconds": python, "ratio": ratio}
        print(f"{name:22} {kestrel * 1000:10.2f} {python * 1000:10.2f} {ratio:7.2f}")
    Path(args.json).write_text(json.dumps({"repetitions": args.repetitions,
                                           "runs": args.runs,
                                           "workloads": results}, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
