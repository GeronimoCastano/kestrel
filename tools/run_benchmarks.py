#!/usr/bin/env python3
"""Build and time the benchmark programs in Kestrel, Rust, Java and Python.

Reports the median wall time of several runs, the peak resident memory of the
fastest run, and the compile time. Every language must print the same output
as Kestrel, except where a program states that it does less work.
"""
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
NAMES = ["numeric", "collections", "strings", "trees", "channels"]
JAVA = {name: name.capitalize() for name in NAMES}
# Python's numeric loop does one tenth of the work; its time is reported as measured.
PARTIAL = {("python", "numeric")}


def timed(command, cwd=ROOT):
    start = time.perf_counter()
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr}")
    return elapsed, result.stdout


def measured_run(command):
    """Wall time from this process; peak memory from /usr/bin/time."""
    timer = ["/usr/bin/time", "-l"] if sys.platform == "darwin" else ["/usr/bin/time", "-v"]
    start = time.perf_counter()
    result = subprocess.run(timer + command, cwd=ROOT, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{result.stderr}")
    if sys.platform == "darwin":
        peak = int(re.search(r"(\d+)\s+maximum resident set size", result.stderr).group(1))
    else:
        peak = int(re.search(r"Maximum resident set size \(kbytes\): (\d+)", result.stderr).group(1)) * 1024
    return elapsed, peak, result.stdout


def build(language, name):
    OUT.mkdir(parents=True, exist_ok=True)
    if language == "kestrel":
        binary = OUT / f"kestrel_{name}"
        seconds, _ = timed([ROOT / "target/release/kestrel", "build", "--root", BENCH / "kestrel", "-o", binary, BENCH / "kestrel" / f"{name}.kes"])
        return seconds, [str(binary)]
    if language == "rust":
        binary = OUT / f"rust_{name}"
        seconds, _ = timed(["rustc", "--edition", "2021", "-C", "opt-level=3", "-C", "codegen-units=1", "-o", binary, BENCH / "rust" / f"{name}.rs"])
        return seconds, [str(binary)]
    if language == "java":
        classes = OUT / "java"
        classes.mkdir(exist_ok=True)
        seconds, _ = timed(["javac", "-d", classes, BENCH / "java" / f"{JAVA[name]}.java"])
        return seconds, ["java", "-cp", str(classes), JAVA[name]]
    return 0.0, [sys.executable, str(BENCH / "python" / f"{name}.py")]


def version(command):
    result = subprocess.run(command, capture_output=True, text=True)
    return (result.stdout or result.stderr).strip().splitlines()[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--only", nargs="*", default=NAMES)
    parser.add_argument("--out", type=Path, default=BENCH / "results.json")
    parser.add_argument("--languages", nargs="*", default=["kestrel", "rust", "java", "python"])
    arguments = parser.parse_args()
    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)
    results = {
        "machine": f"{platform.machine()} {platform.platform()}",
        "versions": {
            "clang": version(["clang", "--version"]),
            "rustc": version(["rustc", "--version"]),
            "java": version(["java", "-version"]),
            "python": version([sys.executable, "--version"]),
        },
        "runs": arguments.runs,
        "benchmarks": {},
    }
    for name in arguments.only:
        rows = {}
        expected = None
        for language in arguments.languages:
            compile_seconds, command = build(language, name)
            times, peaks, output = [], [], None
            # The first launch of a new binary can include a system security scan.
            measured_run(command)
            for _ in range(arguments.runs):
                elapsed, peak, output = measured_run(command)
                times.append(elapsed)
                peaks.append(peak)
            if language == "kestrel":
                expected = output
            elif (language, name) not in PARTIAL and output.strip() != expected.strip():
                raise SystemExit(f"{language} {name} printed different output:\n{output}\nexpected:\n{expected}")
            rows[language] = {
                "seconds": statistics.median(times),
                "peak_bytes": peaks[times.index(min(times))],
                "compile_seconds": compile_seconds,
                "partial": (language, name) in PARTIAL,
            }
            print(f"{name:12} {language:8} {rows[language]['seconds']:8.3f} s  {rows[language]['peak_bytes'] / 2**20:8.1f} MiB  compile {compile_seconds:6.2f} s", flush=True)
        results["benchmarks"][name] = rows
    arguments.out.parent.mkdir(parents=True, exist_ok=True)
    arguments.out.write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
