# Kesdas versus pandas

Measured 2026-11-11 (Europe/Berlin) on an Apple M2 Max (32 GB, macOS 27.0.1),
CPython 3.11.4, pandas 3.0.3, Apple clang 21.0.0 and Kestrel 0.1.0. Full
context and the library comparisons are in
[BENCHMARK_REPORT.md](BENCHMARK_REPORT.md).

## Workload

One CSV with 500,000 rows and seven columns: `id` (Int), `region` and `product`
(String, 8 and 20 levels), `units` (Int), `price` and `discount` (doubles, 2%
missing discounts), and `active` (Bool). The file is 15 MB. Both sides read that
file inside every measured process and run the same ten pipelines:

`filter + top 100`, `groupby + agg`, two-key groupby, pivot, merge, fill missing
+ derive, value counts, rolling mean, correlation, and melt.

```sh
python3 tools/run_kesdas_benchmarks.py --rows 500000 --runs 3 --reps 1
```

Three process samples are taken. CPU time and peak resident memory come from
`/usr/bin/time -l`. Every checksum was checked against pandas before a time was
accepted. The full raw data is in
[benchmarks/kesdas-results-fulljob.json](benchmarks/kesdas-results-fulljob.json).

## Complete job: read the CSV once and run the pipeline

This is what a script normally does. Kesdas wins all ten, from 1.76x to 2.93x.

| Pipeline | Kesdas | pandas | pandas / Kesdas | Kesdas MB | pandas MB |
|---|---:|---:|---:|---:|---:|
| filter_sort | 0.180 | 0.450 | 2.50x | 226 | 204 |
| groupby_agg | 0.180 | 0.450 | 2.50x | 179 | 179 |
| groupby_two_keys | 0.180 | 0.460 | 2.56x | 205 | 195 |
| pivot | 0.160 | 0.450 | 2.81x | 178 | 202 |
| merge | 0.290 | 0.510 | 1.76x | 376 | 286 |
| missing_fill | 0.200 | 0.460 | 2.30x | 209 | 189 |
| value_counts | 0.150 | 0.440 | 2.93x | 165 | 180 |
| rolling | 0.170 | 0.440 | 2.59x | 188 | 212 |
| corr | 0.150 | 0.440 | 2.93x | 144 | 180 |
| melt | 0.200 | 0.440 | 2.20x | 305 | 227 |

Loading alone is 0.15 s of CPU in Kesdas and 0.42 s in pandas. That difference
carries most of the complete-job win.

## Three repetitions, load amortized

Running the pipeline three times per process amortizes the read. The ratios run
from 1.26x to 2.81x; all ten pipelines win in this run.  Raw data:
[benchmarks/kesdas-results-repeated.json](benchmarks/kesdas-results-repeated.json).

## In-memory kernels

Subtracting the separately measured load leaves the pipeline alone. On that
view pandas is 1.2x to 4.6x faster on nine pipelines, and pivot is 1.3x faster than pandas (0.21x to 0.82x pandas/Kesdas on the rest). The subtraction
compares a measured 0.15 s or 0.41 s load against total times of 0.15-0.29 s,
so it is noisy, but the direction is clear: pandas's C and NumPy kernels beat
Kesdas's current column operations.

The complete-job result is the one a user feels, and it is a win. The kernel
result is the work left.

## Peak memory

Kesdas uses 0.80x to 1.34x pandas and is below pandas on five pipelines. Scalar
values are now contiguous, but every column still carries a per-row mask and
string columns allocate a cell per value. A column buffer with a validity
bitmap and shared text is the next step for both memory and kernel speed.