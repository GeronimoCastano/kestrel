#!/usr/bin/env python3
"""Run the Kestrel-versus-Python library benchmarks.

Each suite lives in benchmarks/libraries/<suite>/ with a kestrel/ folder of
programs and a python/ folder of matching programs. A pair with the same file
name is one workload. Both programs print a checksum; the runner compares the
numbers unless the suite lists the workload in timing-only.txt.

    python3 tools/run_library_benchmarks.py --runs 3
    python3 tools/run_library_benchmarks.py --only numpy --runs 3
    python3 tools/run_library_benchmarks.py --suite-dir /tmp/mybench
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
LIBRARIES = ROOT / "benchmarks" / "libraries"
OUT = ROOT / "benchmarks" / "build"
KESTREL = ROOT / "target" / "release" / "kestrel"


def run(command: list[str], cwd: Path = ROOT) -> subprocess.CompletedProcess:
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr[-4000:]}")
    return result


def measured(command: list[str]) -> tuple[float, int, str]:
    start = time.perf_counter()
    result = subprocess.run(["/usr/bin/time", "-l", *command], cwd=ROOT, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr[-4000:]}")
    match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
    if not match:
        raise SystemExit(f"cannot read peak memory for {' '.join(map(str, command))}")
    return elapsed, int(match.group(1)), result.stdout


def numbers(text: str) -> list[float]:
    values = []
    for token in re.findall(r"[-+]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][-+]?\d+)?", text):
        try:
            values.append(float(token))
        except ValueError:
            pass
    return values


def same_checksum(left: str, right: str) -> bool:
    a, b = numbers(left), numbers(right)
    if len(a) != len(b) or not a:
        return False
    for x, y in zip(a, b):
        scale = max(1.0, abs(x), abs(y))
        if abs(x - y) > 1e-6 * scale:
            return False
    return True


def workloads(suite: Path, only: set[str]) -> list[str]:
    names = []
    for source in sorted((suite / "kestrel").glob("*.kes")):
        if source.stem == "baseline":
            continue
        if (suite / "python" / f"{source.stem}.py").exists():
            names.append(source.stem)
    if only:
        names = [name for name in names if name in only]
    return names


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--only", nargs="*", default=None, help="suite names")
    parser.add_argument("--workload", nargs="*", default=None, help="workload names")
    parser.add_argument("--json", default=str(ROOT / "benchmarks" / "library-results.json"))
    args = parser.parse_args()

    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)
    suites = [
        path for path in sorted(LIBRARIES.iterdir())
        if path.is_dir() and (path / "kestrel").is_dir() and (not args.only or path.name in args.only)
    ]
    target = Path(args.json)
    results = None
    if target.exists():
        try:
            results = json.loads(target.read_text())
        except json.JSONDecodeError:
            results = None
    if not isinstance(results, dict) or "suites" not in results:
        results = {"suites": {}}
    results["machine"] = f"{platform.machine()} {platform.platform()}"
    results["runs"] = args.runs
    OUT.mkdir(parents=True, exist_ok=True)
    selected = set(args.workload or [])
    for suite in suites:
        timing_only = set()
        marker = suite / "timing-only.txt"
        if marker.exists():
            timing_only = {line.strip() for line in marker.read_text().splitlines() if line.strip() and not line.startswith("#")}
        rows = {}
        print(f"\n{suite.name}", flush=True)
        baselines = {}
        for language in ("kestrel", "python"):
            if language == "kestrel":
                source = suite / "kestrel" / "baseline.kes"
                if not source.exists():
                    baselines[language] = 0.0
                    continue
                binary = OUT / f"lib_{suite.name}_baseline"
                run([str(KESTREL), "build", "--root", str(suite / "kestrel"), "-o", str(binary), str(source)])
                command = [str(binary)]
            else:
                source = suite / "python" / "baseline.py"
                if not source.exists():
                    baselines[language] = 0.0
                    continue
                command = [sys.executable, str(source)]
            measured(command)
            times = [measured(command)[0] for _ in range(args.runs)]
            baselines[language] = statistics.median(times)
        for name in workloads(suite, selected):
            binary = OUT / f"lib_{suite.name}_{name}"
            run([str(KESTREL), "build", "--root", str(suite / "kestrel"), "-o", str(binary), str(suite / "kestrel" / f"{name}.kes")])
            commands = {
                "kestrel": [str(binary)],
                "python": [sys.executable, str(suite / "python" / f"{name}.py")],
            }
            samples = {}
            outputs = {}
            for language, command in commands.items():
                measured(command)
                times, peaks = [], []
                for _ in range(args.runs):
                    elapsed, peak, output = measured(command)
                    times.append(elapsed)
                    peaks.append(peak)
                samples[language] = {
                    "seconds": statistics.median(times),
                    "minimum_seconds": min(times),
                    "peak_bytes": peaks[times.index(min(times))],
                }
                outputs[language] = output
            if name not in timing_only and not same_checksum(outputs["kestrel"], outputs["python"]):
                raise SystemExit(
                    f"{suite.name}/{name} checksums differ:\nKestrel:\n{outputs['kestrel']}\nPython:\n{outputs['python']}"
                )
            k, p = samples["kestrel"], samples["python"]
            k_compute = max(0.0, k["seconds"] - baselines["kestrel"])
            p_compute = max(0.0, p["seconds"] - baselines["python"])
            compute_speedup = p_compute / k_compute if k_compute > 0 else 0.0
            rows[name] = {
                "kestrel": k,
                "python": p,
                "kestrel_startup": baselines["kestrel"],
                "python_startup": baselines["python"],
                "kestrel_compute": k_compute,
                "python_compute": p_compute,
                "speedup": p["seconds"] / k["seconds"],
                "compute_speedup": compute_speedup,
                "memory_ratio": k["peak_bytes"] / p["peak_bytes"],
                "timing_only": name in timing_only,
            }
            print(
                f"  {name:24} kestrel {k['seconds']:7.3f} s {k['peak_bytes'] / 2**20:7.1f} MiB   "
                f"python {p['seconds']:7.3f} s {p['peak_bytes'] / 2**20:7.1f} MiB   "
                f"wall {p['seconds'] / k['seconds']:5.2f}x   compute {compute_speedup:5.2f}x   "
                f"memory {k['peak_bytes'] / p['peak_bytes']:5.2f}x",
                flush=True,
            )
        results["suites"][suite.name] = {"baselines": baselines, "workloads": rows}

    Path(args.json).write_text(json.dumps(results, indent=2) + "\n")
    print(f"\nwrote {args.json}")


if __name__ == "__main__":
    main()
