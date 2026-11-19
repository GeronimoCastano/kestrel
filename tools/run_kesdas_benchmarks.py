#!/usr/bin/env python3
"""Compare Kesdas and pandas pipelines on the same CSV data.

    python3 tools/run_kesdas_benchmarks.py --rows 500000 --runs 5

Both sides read the same CSV inside every measured process. CPU time is the
child's user+sys time, which is steadier than wall time on a busy machine.
Peak memory is the process high-water mark from /usr/bin/time. The `load`
pipeline is measured separately so the report can show total and estimated
compute-only time.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KESTREL_COMPILER = ROOT / "target/debug/kestrel"
KESTREL_PROGRAM = ROOT / "benchmarks/kesdas/pipelines.kes"
KESTREL_BINARY = ROOT / "benchmarks/build/kesdas_pipelines"
PANDAS_PROGRAM = ROOT / "benchmarks/python/kesdas_pipelines.py"
TIME = shutil.which("time") or "/usr/bin/time"

PIPELINES = [
    "filter_sort",
    "groupby_agg",
    "groupby_two_keys",
    "pivot",
    "merge",
    "missing_fill",
    "value_counts",
    "rolling",
    "corr",
    "melt",
]


def generate(path: Path, rows: int) -> None:
    if path.is_file():
        return
    import numpy as np
    import pandas as pd

    print(f"generating {rows:,} rows in {path}")
    rng = np.random.default_rng(42)
    regions = np.array([f"r{i}" for i in range(8)])
    products = np.array([f"p{i}" for i in range(20)])
    frame = pd.DataFrame(
        {
            "id": np.arange(rows, dtype=np.int64),
            "region": rng.choice(regions, rows),
            "product": rng.choice(products, rows),
            "units": rng.integers(1, 20, rows, dtype=np.int64),
            "price": np.round(rng.uniform(1, 8, rows), 2),
            "discount": np.round(rng.uniform(0, 0.2, rows), 2),
            "active": rng.random(rows) < 0.6,
        }
    )
    frame.loc[rng.random(rows) < 0.02, "discount"] = np.nan
    frame.to_csv(path, index=False, na_rep="")


def build() -> None:
    KESTREL_BINARY.parent.mkdir(parents=True, exist_ok=True)
    print("building the Kesdas benchmark binary")
    subprocess.run(
        [str(KESTREL_COMPILER), "build", str(KESTREL_PROGRAM), "-o", str(KESTREL_BINARY)],
        check=True,
    )


REAL = re.compile(r"([0-9.]+)\s+real\s+([0-9.]+)\s+user\s+([0-9.]+)\s+sys")
RSS_MAC = re.compile(r"(\d+)\s+maximum resident set size")
USER_LINUX = re.compile(r"User time \(seconds\):\s*([0-9.]+)")
SYS_LINUX = re.compile(r"System time \(seconds\):\s*([0-9.]+)")
RSS_LINUX = re.compile(r"Maximum resident set size \(kbytes\):\s*(\d+)")


def measure(command: list[str]) -> tuple[float, float, int, str]:
    started = time.perf_counter()
    done = subprocess.run(
        [TIME, "-l", *command],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    elapsed = time.perf_counter() - started
    if done.returncode != 0:
        raise RuntimeError(
            f"command failed ({done.returncode}): {' '.join(command)}\n{done.stdout}\n{done.stderr}"
        )
    text = done.stderr
    match = REAL.search(text)
    if match:
        cpu = float(match.group(2)) + float(match.group(3))
    else:
        user = USER_LINUX.search(text)
        sys_time = SYS_LINUX.search(text)
        if not user or not sys_time:
            raise RuntimeError(f"cannot read CPU time from time output:\n{text}")
        cpu = float(user.group(1)) + float(sys_time.group(1))
    rss_match = RSS_MAC.search(text)
    rss = int(rss_match.group(1)) if rss_match else 0
    if not rss:
        rss_match = RSS_LINUX.search(text)
        rss = int(rss_match.group(1)) * 1024 if rss_match else 0
    lines = [line.strip() for line in done.stdout.splitlines() if line.strip()]
    checksum = lines[-1] if lines else ""
    return cpu, elapsed, rss, checksum


def command_for(kind: str, data: Path, pipeline: str, reps: int) -> list[str]:
    if kind == "kestrel":
        return [str(KESTREL_BINARY), str(data), pipeline, str(reps)]
    return [sys.executable, str(PANDAS_PROGRAM), str(data), pipeline, str(reps)]


def median(values: list[float]) -> float:
    return statistics.median(values)


def collect(kind: str, data: Path, pipeline: str, runs: int, reps: int) -> dict:
    # One untimed warm-up, then the measured runs.
    measure(command_for(kind, data, pipeline, reps))
    samples = [measure(command_for(kind, data, pipeline, reps)) for _ in range(runs)]
    return {
        "cpu": median([sample[0] for sample in samples]),
        "wall": median([sample[1] for sample in samples]),
        "rss": max(sample[2] for sample in samples),
        "checksum": samples[-1][3],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rows", type=int, default=500_000)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--data", type=Path)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    data = args.data or ROOT / f"benchmarks/build/kesdas-data-{args.rows}.csv"
    data.parent.mkdir(parents=True, exist_ok=True)
    generate(data, args.rows)
    if not args.skip_build:
        build()

    load_k = collect("kestrel", data, "load", args.runs, 1)
    load_p = collect("pandas", data, "load", args.runs, 1)
    if not math.isclose(float(load_k["checksum"]), float(load_p["checksum"]), rel_tol=1e-9):
        raise SystemExit(f"load checksums differ: {load_k['checksum']} vs {load_p['checksum']}")
    print(f"load: Kesdas {load_k['cpu']:.2f}s CPU, pandas {load_p['cpu']:.2f}s CPU")
    print()

    results = []
    for pipeline in PIPELINES:
        print(f"running {pipeline} ...", flush=True)
        k = collect("kestrel", data, pipeline, args.runs, args.reps)
        p = collect("pandas", data, pipeline, args.runs, args.reps)
        want = float(k["checksum"])
        got = float(p["checksum"])
        if not math.isclose(want, got, rel_tol=1e-8, abs_tol=1e-8):
            raise SystemExit(
                f"{pipeline} checksums differ: Kesdas {want!r}, pandas {got!r}"
            )
        entry = {
            "pipeline": pipeline,
            "kestrel": k,
            "pandas": p,
            "kestrel_compute": max(0.0, (k["cpu"] - load_k["cpu"]) / args.reps),
            "pandas_compute": max(0.0, (p["cpu"] - load_p["cpu"]) / args.reps),
            "kestrel_total": k["cpu"] / args.reps,
            "pandas_total": p["cpu"] / args.reps,
            "speed_ratio_total": p["cpu"] / k["cpu"] if k["cpu"] else float("inf"),
            "speed_ratio_compute": (
                max(0.0, p["cpu"] - load_p["cpu"])
                / max(1e-9, k["cpu"] - load_k["cpu"])
            ),
            "memory_ratio": p["rss"] / k["rss"] if k["rss"] else float("inf"),
        }
        results.append(entry)
        print(
            f"  total  Kesdas {entry['kestrel_total']:.3f}s  pandas {entry['pandas_total']:.3f}s"
            f"  ({entry['speed_ratio_total']:.2f}x pandas/Kesdas)"
        )
        print(
            f"  compute Kesdas {entry['kestrel_compute']:.3f}s  "
            f"pandas {entry['pandas_compute']:.3f}s  "
            f"({entry['speed_ratio_compute']:.2f}x)"
        )
        print(
            f"  peak   Kesdas {k['rss'] / 1e6:.1f} MB  pandas {p['rss'] / 1e6:.1f} MB"
        )

    output = {
        "rows": args.rows,
        "runs": args.runs,
        "reps": args.reps,
        "data": str(data),
        "load": {"kestrel": load_k, "pandas": load_p},
        "pipelines": results,
    }
    target = ROOT / "benchmarks/kesdas-results.json"
    target.write_text(json.dumps(output, indent=2) + "\n")
    print(f"\nwrote {target}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
