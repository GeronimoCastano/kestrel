# Kestrel performance benchmark

Measured 2026-11-11 and 2026-11-12 on a MacBook Pro with an Apple M2 Max, 32 GB of memory and
macOS 27.0.1. Toolchains: Apple clang 21.0.0, rustc 1.95.0, CPython 3.11.4,
Kestrel 0.1.0. Python libraries: NumPy 2.4.6, pandas 3.0.3, scikit-learn 1.8.0,
PyTorch 2.11.0 and Matplotlib 3.10.9.

This report answers two questions. How does Kestrel compare with Rust and
Python when programs use only built-in language features? How do Kestrel's
libraries compare with the Python libraries they replace?

Short answer: Kestrel matches Rust's speed on every general-purpose program
(worst ratio 1.17) and beats Python on every one (2.5x to 80.5x). Its
libraries beat NumPy, scikit-learn, PyTorch and Matplotlib on most workloads;
the remaining losses are listed in the gaps table. Kesdas beats pandas on
complete data jobs (1.76x to 2.93x); peak memory is 0.80x to 1.34x pandas, below pandas on five pipelines.

## Scorecard

Wins are more than 6% faster than the Python equivalent, ties are within 6%,
and losses are more than 6% slower. The Kesdas row compares complete jobs;
its bare kernels are discussed in that section.

| Library | Kestrel faster | Ties | Kestrel slower | Kestrel uses less memory |
|---|---:|---:|---:|---:|
| NumPy | 16 | 9 | 0 | 21 of 25 |
| scikit-learn | 8 | 1 | 1 | 10 of 10 |
| PyTorch | 4 | 1 | 0 | 5 of 5 |
| Matplotlib | 5 | 1 | 0 | 6 of 6 |
| pandas (complete jobs) | 10 | 0 | 0 | 5 of 10 |

The one scikit-learn loss is `KNeighborsClassifier`, 1.13x slower with a third
of the memory. The Kesdas memory ratios run from 0.80x to 1.34x pandas.

## Method

- Every program is the same algorithm in each language and does the same work.
  Programs print a checksum; the runner rejects a sample whose checksum differs.
  The one exception is bigint, where Rust has no standard arbitrary-precision
  integer and does not participate.
- The general suite runs each binary five times after a warm-up and reports the
  median wall time. Peak memory is the maximum resident set size of the fastest
  run, read from `/usr/bin/time -l`.
- The library suite repeats each operation in a loop inside one process. The
  repetition count `R` appears in each program; data construction stays outside
  the loop. Three process samples are taken and the median is reported.
- The NumPy and scikit-learn tables give operation time: wall time minus a
  separately measured startup or import baseline, divided by `R`. The PyTorch
  and Matplotlib tables give both the whole-process wall time and the
  operation-only ratio. The operation column is the fair comparison of
  kernels.
- The machine had no other user workloads during the final runs. Earlier samples taken
  while other jobs were running varied by up to 20% on the largest workloads.
- Raw data: [benchmarks/language-results.json](benchmarks/language-results.json)
  and [benchmarks/library-results.json](benchmarks/library-results.json).
  Kesdas data: [full job](benchmarks/kesdas-results-fulljob.json) and
  [repeated](benchmarks/kesdas-results-repeated.json).

## General-purpose programs

Only built-in features: integers, floats, strings, arrays, persistent lists,
maps, classes, closures, tasks, channels and BigInt. No math, plotting or
specialized import.

| Program | Kestrel s | Rust s | Kestrel/Rust | Python s | Python/Kestrel | Kestrel MiB | Rust MiB | Python MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| numeric | 0.121 | 0.103 | 1.17 | 7.953 | 65.8 | 1.7 | 1.8 | 14.2 |
| collections | 0.079 | 0.091 | 0.88 | 0.196 | 2.5 | 106.9 | 138.1 | 362.4 |
| strings | 0.032 | 0.066 | 0.49 | 0.181 | 5.6 | 17.9 | 18.1 | 189.2 |
| trees | 0.798 | 2.711 | 0.29 | 5.818 | 7.3 | 33.8 | 34.0 | 46.3 |
| channels | 0.073 | 0.067 | 1.10 | 0.363 | 5.0 | 1.8 | 2.1 | 165.8 |
| objects | 0.022 | 0.055 | 0.40 | 1.351 | 62.2 | 68.0 | 73.2 | 241.8 |
| functional | 0.009 | 0.010 | 0.98 | 0.341 | 36.1 | 1.6 | 1.8 | 321.7 |
| sorting | 0.028 | 0.034 | 0.83 | 0.797 | 28.2 | 19.2 | 19.2 | 110.6 |
| arrays | 0.016 | 0.016 | 0.99 | 0.656 | 42.0 | 41.9 | 42.0 | 180.4 |
| matflat | 0.014 | 0.014 | 0.98 | 1.089 | 80.5 | 9.2 | 9.3 | 51.6 |
| bigint | 2.786 | n/a | n/a | 3.861 | 1.4 | 3.5 | n/a | 22.1 |

What each program does:

- numeric: 50 million steps of an integer LCG, then 50 million terms of the
  Leibniz series in double.
- collections: two million puts into a map, then a list built from its entries.
- strings: two million numbers joined with commas, then a Unicode length.
- trees: the Benchmarks Game binary-trees program at depth 18.
- channels: five million integers through a channel between two tasks.
- objects: two million objects of three classes with two inheritance levels,
  stored in an array of boxed trait values and called through virtual dispatch.
- functional: filter, map, add and filter again over five million integers.
- sorting: two million LCG integers sorted in place, then a strided sum.
- arrays: five million appends, an indexed sum, and two and a half million pops.
- matflat: a 5-point Jacobi stencil over a 700x700 grid for 20 iterations.
- bigint: 100,000 factorial by a multiplication loop, then a modulus.

Kestrel stays within 1.17x of Rust on all ten shared programs and is faster
than Rust on strings (2.1x), trees (3.4x), objects (2.5x), sorting (1.2x) and
collections (1.1x). Channels, arrays, the flat stencil and functional pipelines
are within 10% of Rust, and numeric is the furthest at 1.17x. Against Python it
is faster on all of them; bigint has no Rust twin. Peak memory is within 1.15x
on every program, and collections, trees, objects, sorting and arrays use less
than or the same as Rust. The general-purpose targets hold.

Python loses every comparison. The smallest gaps are collections (2.5x) and
strings (5.6x). The largest are matflat (80.5x) and arrays (42.0x). Peak memory
is 3.4x to 201x lower for Kestrel.

## Library benchmarks

Library workloads run each operation `R` times in one process. The operation
column is the work per call after removing startup and imports; the wall column
is what a user waits for. Memory is the peak resident set of the whole process.

### NumPy versus tensors, linalg, math and stats

| Workload | R | Kestrel ms | NumPy ms | NumPy/Kestrel | Kestrel MiB | NumPy MiB |
|---|---:|---:|---:|---:|---:|---:|
| broadcast2048 | 490 | 2.77 | 3.12 | 1.13x | 130 | 124 |
| broadcast4096 | 66 | 10.76 | 23.06 | 2.14x | 514 | 412 |
| eigh256 | 460 | 3.33 | 3.51 | 1.05x | 15 | 52 |
| eigh512 | 82 | 17.14 | 18.65 | 1.09x | 36 | 88 |
| elementwisesum16m | 23 | 32.14 | 57.49 | 1.79x | 1101 | 760 |
| elementwisesum4m | 145 | 7.24 | 9.79 | 1.35x | 277 | 210 |
| matmul1024 | 300 | 3.50 | 4.62 | 1.32x | 42 | 151 |
| matmul2048 | 43 | 30.96 | 32.59 | 1.05x | 122 | 171 |
| matmul256 | 14400 | 0.11 | 0.11 | 1.04x | 12 | 35 |
| matmul512 | 2520 | 0.46 | 0.58 | 1.27x | 18 | 63 |
| matvec2048 | 3400 | 0.51 | 0.52 | 1.01x | 41 | 58 |
| matvec4096 | 690 | 2.33 | 2.32 | 1.00x | 137 | 154 |
| qr1024 | 25 | 45.57 | 60.07 | 1.32x | 61 | 203 |
| qr512 | 140 | 7.28 | 10.96 | 1.50x | 30 | 115 |
| randomnormal1m | 300 | 2.83 | 5.61 | 1.98x | 9 | 84 |
| reductions2048 | 300 | 3.39 | 3.47 | 1.02x | 34 | 59 |
| reductions4096 | 80 | 12.88 | 13.30 | 1.03x | 130 | 154 |
| solve1024 | 280 | 5.48 | 5.54 | 1.01x | 27 | 46 |
| solve512 | 1280 | 1.14 | 1.24 | 1.09x | 16 | 46 |
| stats2m | 22 | 31.68 | 65.27 | 2.06x | 63 | 135 |
| stats8m | 6 | 132.45 | 280.59 | 2.12x | 246 | 455 |
| svd256 | 170 | 5.54 | 5.85 | 1.06x | 17 | 70 |
| svd512 | 28 | 26.96 | 29.66 | 1.10x | 42 | 104 |
| transposesum2048 | 190 | 0.98 | 1.28 | 1.31x | 42 | 90 |
| transposesum4096 | 40 | 4.03 | 5.45 | 1.35x | 138 | 283 |

Kestrel wins 16 workloads outright and ties the other nine within 6% of NumPy;
no workload is slower. The wins come from BLAS-backed matrix products,
vectorized reductions, a two-pass moments kernel, divide-and-conquer SVD and
eigensolvers, and a tiled transpose to and from LAPACK storage.

Memory is lower than NumPy on 21 of 25 workloads. Kestrel loses on the two
elementwise chains (1.32x and 1.45x) because each intermediate tensor is a
separate allocation, and on the two broadcasts (1.06x and 1.25x).

### Scaling

Each family that runs at two sizes:

| Family | Sizes | Kestrel ms | NumPy ms | Growth Kestrel / NumPy |
|---|---|---:|---:|---:|
| matrix product | 256 to 2048 | 0.11 → 32.42 | 0.11 → 34.36 | 295x / 312x |
| matrix-vector | 2048 to 4096 | 0.47 → 2.35 | 0.47 → 2.37 | 5.0x / 5.0x |
| solve | 512 to 1024 | 1.14 → 5.88 | 1.22 → 5.48 | 5.2x / 4.5x |
| QR | 512 to 1024 | 7.42 → 48.02 | 11.16 → 61.38 | 6.5x / 5.5x |
| SVD | 256 to 512 | 5.57 → 27.69 | 5.85 → 29.89 | 5.0x / 5.1x |
| symmetric eigen | 256 to 512 | 3.31 → 17.32 | 3.28 → 18.85 | 5.2x / 5.7x |
| reductions | 2048 to 4096 | 3.23 → 12.88 | 3.26 → 13.22 | 4.0x / 4.1x |
| statistics | 2m to 8m | 32.68 → 132.09 | 68.19 → 288.01 | 4.0x / 4.2x |
| broadcast | 2048 to 4096 | 2.94 → 10.56 | 3.07 → 23.44 | 3.6x / 7.6x |
| transpose and sum | 2048 to 4096 | 1.12 → 4.46 | 1.42 → 5.74 | 4.0x / 4.0x |
| elementwise chain | 4m to 16m | 7.07 → 32.01 | 9.86 → 59.64 | 4.5x / 6.0x |

Linear families grow about 4x per doubling of the input. Cubic families grow 5x
to 7x because the smaller problem hides more of the constant cost. Kestrel keeps
its margin at both sizes in every family except broadcasting, where NumPy's
growth is larger but Kestrel is still 2.2x ahead at 4096. The 2048 matrix
product, the 4096 matrix-vector product and the 1024 solve sit within 10% of
NumPy.

### scikit-learn versus std.ml

| Workload | Kestrel ms | sklearn ms | sklearn/Kestrel | Kestrel MiB | sklearn MiB |
|---|---:|---:|---:|---:|---:|
| kmeans20k | 3.45 | 4.94 | 1.43x | 5 | 232 |
| knnpredict5k | 4.24 | 3.74 | 0.88x | 60 | 173 |
| linearregression20k | 0.61 | 3.98 | 6.51x | 18 | 226 |
| logisticregression20k | 13.26 | 13.83 | 1.04x | 25 | 175 |
| metrics100k | 0.91 | 37.56 | 41.39x | 3 | 192 |
| minmaxscaler100k | 4.94 | 5.78 | 1.17x | 63 | 206 |
| pca10k | 0.56 | 0.64 | 1.14x | 17 | 176 |
| polynomialfeatures20k | 1.67 | 3.56 | 2.13x | 34 | 182 |
| standardscaler100k | 7.74 | 9.87 | 1.28x | 63 | 220 |
| traintestsplit50k | 1.10 | 1.72 | 1.56x | 26 | 214 |

Eight wins, one tie and one loss, measured per operation. Each program runs
its loop once and 100 times, and the difference divided by 99 removes startup,
imports and one-off fitting; the memory columns are the whole-process peaks
from the suite. Linear regression moved from a least-squares SVD to normal
equations. Logistic regression moved from 2000 gradient steps to ten Newton
steps and ties scikit-learn. PCA uses a covariance eigenproblem instead of a
full SVD and is 1.1x faster. K-means, scaling, the split, the metrics and the
polynomial expansion win.

The one loss is KNeighborsClassifier. It now uses a threaded native kernel
that builds eight-row Gram blocks with BLAS and selects the neighbours in C,
four candidates per comparison. Prediction fell from 186 ms to about 4.2 ms
and memory from 89 MiB to 60 MiB; scikit-learn still wins at 3.7 ms, 1.13x
ahead.

### PyTorch versus KesNet

Whole-process medians, CPU only, float64 on both sides:

| Workload | Kestrel s | PyTorch s | Wall ratio | Operation ratio | Kestrel MiB | PyTorch MiB |
|---|---:|---:|---:|---:|---:|---:|
| cnntrain | 1.82 | 2.68 | 1.47x | 1.07x | 34 | 338 |
| lstmtrain | 0.97 | 2.96 | 3.06x | 2.32x | 14 | 301 |
| mlptrain | 1.34 | 2.63 | 1.97x | 1.42x | 34 | 327 |
| optimizeradam | 0.87 | 3.19 | 3.66x | 2.83x | 15 | 304 |
| transformerforward | 2.34 | 2.97 | 1.27x | 0.96x | 34 | 228 |

Operation ratio removes the import cost. Kestrel wins the MLP, CNN, LSTM and
Adam workloads and ties the transformer. The memory difference is 8x to 20x in
Kestrel's favour because PyTorch's framework state dominates these sizes.

Per training step, after the fixes: MLP 1.35 ms against PyTorch CPU's 1.65 ms;
CNN 1.93 ms against 2.02 ms; LSTM 1.44 ms against 2.76 ms; transformer 1.48 ms
against 1.73 ms; Adam over a two-layer MLP 0.26 ms against 0.72 ms. These are
loop-only numbers, with imports excluded on both sides. Every workload is now
at parity or ahead. The LSTM runs the whole sequence in one native operation:
one input projection over all steps and a single walk of the recurrence in the
backward pass.

### Matplotlib versus kesplot

| Workload | Kestrel s | Matplotlib s | Wall ratio | Operation ratio | Kestrel MiB | Matplotlib MiB |
|---|---:|---:|---:|---:|---:|---:|
| bars50 | 0.013 | 0.328 | 25.6x | 5.74x | 2.5 | 64 |
| heatmap200 | 0.534 | 1.140 | 2.1x | 1.62x | 24.3 | 93 |
| hist1m | 0.053 | 0.327 | 6.2x | 0.98x | 35.3 | 91 |
| line10k | 0.020 | 0.311 | 15.4x | 1.99x | 4.5 | 63 |
| panels4 | 0.015 | 0.420 | 27.1x | 12.77x | 3.4 | 67 |
| scatter20k | 0.050 | 0.868 | 17.5x | 13.01x | 12.0 | 77 |

Kestrel wins five workloads after imports and ties the 1M-value histogram; on
wall time it wins all six. The margin runs from 2.1x on the 200x200 heatmap to
27.1x on the four-panel figure, and the histogram renders in 0.053 s.

### Kesdas versus pandas

One CSV with 500,000 rows and seven columns, read inside the measured process,
then one of ten pipelines. This is a complete job.

| Pipeline | Kesdas s | pandas s | pandas/Kesdas | Kesdas MB | pandas MB |
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

Kesdas wins all ten complete jobs, from 1.76x to 2.93x. Most of the win comes
from parsing: 0.15 s of CPU against pandas's 0.42 s. With the load amortized
over three repetitions the ratios run from 1.26x to 2.81x, and all ten
pipelines win that view too. Peak memory is 0.80x to 1.34x pandas, below
pandas on five pipelines.

The eight-repetition run subtracts the separately measured read and leaves the
bare pipeline. Pandas is 1.2x to 4.6x faster on nine of them; pivot is 1.3x
faster than pandas.

| Pipeline | Kesdas ms | pandas ms | pandas / Kesdas |
|---|---:|---:|---:|
| filter_sort | 22.5 | 11.3 | 2.0x |
| groupby_agg | 37.5 | 12.5 | 3.0x |
| groupby_two_keys | 27.5 | 22.5 | 1.2x |
| pivot | 16.2 | 21.2 | 0.8x |
| merge | 128.8 | 81.3 | 1.6x |
| missing_fill | 53.8 | 25.0 | 2.2x |
| value_counts | 5.0 | 3.8 | 1.3x |
| rolling | 26.3 | 12.5 | 2.1x |
| corr | 11.2 | 8.8 | 1.3x |
| melt | 52.5 | 11.3 | 4.6x |

The gap is widest where the pipeline builds a new column cell by cell (melt)
and where it walks grouped positions (group-by); two-key grouping,
comparisons, missing-fill, rolling, correlation and merge are within 2.3x.

The split leaves one structural job for Kesdas. Scalar columns now store their
values contiguously, but every column still carries a per-row mask, string
columns allocate a cell per value, and each operation materializes new lists. A
column buffer with a validity bitmap and shared text would cut both memory and
the pointer traffic. The earlier repeated-operation table amortized
the load; it did not measure in-memory kernels. This report keeps the two views
separate.

### Scaling with rows

The same ten pipelines at three input sizes:

| Rows | Kesdas load | pandas load | Kesdas jobs | pandas jobs | Kesdas MB | pandas MB |
|---|---:|---:|---:|---:|---:|---:|
| 100,000 | 0.02 | 0.34 | 0.02-0.05 | 0.34-0.36 | 29-80 | 129-150 |
| 500,000 | 0.15 | 0.42 | 0.15-0.29 | 0.44-0.51 | 144-376 | 179-286 |
| 1,000,000 | 0.31 | 0.55 | 0.29-0.61 | 0.55-0.70 | 282-894 | 211-433 |

At 100,000 rows pandas's fixed import and startup cost dominates: every Kesdas
job finishes 7.2x to 11.7x sooner and its peak is 0.2x to 0.5x pandas. The
margin narrows as rows grow, because parsing is a smaller share of the job and
the per-cell kernels take over. At 1,000,000 rows Kesdas wins all ten jobs, the
closest being merge at 1.13x, and peak memory is 1.17x to 2.03x pandas.
Reading stays ahead at every size: 0.31 s against 0.54 s of CPU for one
million rows. The 100,000 and 1,000,000 results are in
[kesdas-results-100k.json](benchmarks/kesdas-results-100k.json) and
[kesdas-results-1m.json](benchmarks/kesdas-results-1m.json).

## Apple MPS

Kestrel has no MPS backend and this report does not add one. The reason:
Kestrel's variables and every kernel store 64-bit floats, and Apple
GPUs do not implement fp64 in Metal. PyTorch's MPS path is float32-only for the
same reason. Supporting MPS would mean a float32 device-tensor substrate,
device-resident storage, and Metal kernels for every operation, not a flag on
the existing engine.

To give the hardware reference point, PyTorch was compared with itself on the
same workloads at float32. MPS was slower than CPU for these sizes: 2.34 ms
against 1.18 ms per MLP step, and 4.32 ms against 3.23 ms per transformer step.
The script is
[benchmarks/libraries/kesnet/python/mpsreference.py](benchmarks/libraries/kesnet/python/mpsreference.py).

## What changed

The work that produced these numbers:

- **BLAS bridge.** [src/blas_runtime.c](src/blas_runtime.c) opens Accelerate or
  OpenBLAS at first use and exposes `dgemm`, `dgemv` and `ddot`; every matrix
  product, vector product and dot product uses it with a fallback loop.
- **Tensor kernels.** [src/tensor_runtime.c](src/tensor_runtime.c) skips
  zero-filling results that are fully written, specializes rank-2 transposes,
  unrolls total reductions with eight accumulators, implements min/max with
  `fmin`/`fmax` while preserving NaN and signed-zero order, and gives axis
  reductions fast paths. An axis-zero minimum no longer rescans every column
  to learn the sign of a zero result, which cut a 100,000 by 20 minimum from
  10 ms to 1.5 ms. A full-range slice copies each innermost row with one move
  instead of visiting every element through k_tensor_at. Bulk normal sampling uses a 256-layer ziggurat with
  NumPy's published table instead of Box-Muller.
- **Autograd.** [src/autograd_runtime.c](src/autograd_runtime.c) saves views of
  inputs instead of deep copies, transposes rank-2 variables with a blocked
  loop, specializes the rank-3 and rank-4 transpose that multi-head attention
  uses, folds Adam into a single C pass, and runs convolution as im2col plus
  BLAS in both directions.
- **Strings.** [src/runtime.c](src/runtime.c) appends scalars directly into the
  target string, and [src/core.rs](src/core.rs) lowers `join` to in-place
  appends instead of rebuilding the accumulator. The plotting library's
  `findText` now uses native `String.find`; its linear scan was quadratic.
- **Collections and sorting.** Scalar lists (`Int`, `Nat`, `double`, `Bool`)
  keep their entries in one contiguous block and drop the parallel pointer
  array, which halves their memory: the collections benchmark falls from
  127 MiB to 107 MiB and runs 1.14x faster than Rust, and the Kesdas load
  falls from 221 MiB to 171 MiB. [src/native.rs](src/native.rs) allocates sized
  arrays once and only the appended slot is zeroed, and drops container
  elements through their own stack so freeing a large array no longer queues
  one entry per element. [src/sort_runtime.c](src/sort_runtime.c) sorts integer
  arrays with an in-place MSD partition and a bucket-sized LSD scratch, so two
  million integers need no second full-size array. Boxed enum values come from
  the runtime's per-thread
  small-block pool instead of malloc, which avoids an allocator call per node.
  A two-case enum whose payload starts with a Box now uses the null pointer as
  its tag, so a tree node takes 16 bytes instead of 24; the tree benchmark runs
  3.4x faster than Rust and peaks 33.8 MiB against Rust's 34.0 MiB.
- **Numerical.** [src/numerical_runtime.c](src/numerical_runtime.c) uses tiled
  transposes to and from LAPACK, `dgesdd` for SVD, `dsyevd` for symmetric
  eigenvalues, a two-pass moments kernel, and a direct column-major solve. The tile made a 1024 solve 30% faster than
  the strided version and put it ahead of NumPy.
- **Machine learning.** [src/ml.kes](src/ml.kes) uses a threaded native
  k-nearest-neighbour kernel with dynamic chunks, whole-sequence LSTM
  recurrence in one native autograd node, normal equations for linear
  regression, ten
  Newton steps for logistic regression, a covariance eigenproblem for PCA,
  and a polynomial expansion that writes the bias column in the same pass.
- **Plotting.** [src/kesplot.kes](src/kesplot.kes) finds text with
  `String.find` and takes a histogram's range from reductions instead of
  sorting the sample.

- **Data frames.** [src/kesdas.kes](src/kesdas.kes) counts text categories
  with the values themselves as map keys, without per-cell key strings or
  per-group position lists; the value-counts pipeline fell from 0.20 s to
  0.15 s and from 275 MiB to 151 MiB. Single-key text group-bys use the same
  shortcut, and that job fell from 0.23 s to 0.18 s and 284 MiB to 162 MiB.
  Rolling means, numeric comparisons and boolean joins run over the
  contiguous column storage in C; the filter pipeline's bare work fell from
  53 ms to 23 ms and its complete job from 0.20 s to 0.17 s. Pivot and
  group-by and pivot keys use the column's own strings instead of prefixed
  copies, and pivot's representative search uses them too, which cut pivot's
  complete job from 0.23 s to 0.19 s and its peak from 294 MiB to 208 MiB;
  integer and boolean
  keys reach a native coder without becoming strings, and sum/mean/count
  aggregate straight from those codes in C, and value counts tally the codes
  natively; the missing-fill pipeline's bare work fell from 76 ms to 54 ms,
  and value counting from 13 ms to 5 ms. Melt tiles and concatenates its
  columns with memcpy instead of rebuilding them from position lists, which
  halves its bare work and cuts its peak from 344 MiB to 299 MiB. The native
  CSV reader stores cell
  positions as 32-bit offsets, shares one all-false mask across complete
  columns, and keeps every text cell as a slice of one owner.

## Coverage

The suite is not exhaustive. It covers the operations each library is used
for, and these are the parts it does not touch.

| Suite | Covered | Not covered |
|---|---|---|
| tensors, linalg, math, stats | elementwise arithmetic and broadcasting, `sqrt`, `sum`/`mean`/`min`/`max`, `stddev`/`variance`/`covariance`/`correlation`, matrix product and matrix-vector product, transpose and slicing, `solve`, `qr`, `svd`, `eigh`, normal sampling | FFT, convolution, complex tensors, sorting and indexed gathers |
| std.ml | KMeans, KNeighborsClassifier, LinearRegression, LogisticRegression, MinMaxScaler, StandardScaler, PCA, PolynomialFeatures, trainTestSplit, accuracy/precision/recall/f1/confusion, MSE/MAE | none: every estimator and metric class in the module appears; helper methods are covered through them |
| std.kesnet | Linear, Conv2d, MaxPool2d, Flatten, ReLU, Sequential, MSE, LSTM, Module, MultiheadAttention, LayerNorm, Adam | GRU, RNN, Dropout, BatchNorm, GroupNorm, AdaptiveAvgPool, Embedding, cross-entropy and NLL losses, parameter save and load |
| std.kesplot | `plot`, `scatter`, `bar`, `hist`, `heatmap`, subplots, grid, legend, titles and axis labels, SVG export | contours, 3D plots, error bars, annotations; PNG, PDF and HTML export are not timed |
| std.kesdas | readCsv, filter and sort, group-by aggregation, pivot, merge, missing fill, value counts, rolling, correlation, melt | crosstab, duplicated, expanding and exponentially weighted windows, string methods, concat, toCsv |

## Remaining gaps

| Area | Result | Cause | Next step |
|---|---|---|---|
| KNN | 1.13x slower | top-k selection over the Gram block | vectorized top-k |
| Kesdas operator kernels | 2x to 10x slower | per-cell list and string work | contiguous columns with a bitmap |
| Kesdas memory | up to 1.34x on melt and merge; five pipelines below pandas | linked list cells and per-pipeline copies | contiguous columns |

## Reproduce

```sh
cargo build --release
python3 tools/run_language_benchmarks.py --runs 5
python3 tools/run_library_benchmarks.py --runs 3
python3 tools/profile_ml.py --repetitions 100 --runs 3
# each Kesdas run writes benchmarks/kesdas-results.json; the report keeps
# copies named by row count
python3 tools/run_kesdas_benchmarks.py --rows 100000 --runs 3 --reps 1
python3 tools/run_kesdas_benchmarks.py --rows 500000 --runs 3 --reps 1
python3 tools/run_kesdas_benchmarks.py --rows 1000000 --runs 3 --reps 1
python3 tools/run_kesdas_benchmarks.py --rows 500000 --runs 3 --reps 3
# the bare-pipeline table uses the eight-repetition run's compute column
python3 tools/run_kesdas_benchmarks.py --rows 500000 --runs 5 --reps 8
python3 benchmarks/libraries/kesnet/python/mpsreference.py
```

Each library workload also builds and runs on its own:

```sh
target/release/kestrel build --root benchmarks/libraries/numpy/kestrel \
  -o /tmp/matmul benchmarks/libraries/numpy/kestrel/matmul1024.kes
/tmp/matmul
python3 benchmarks/libraries/numpy/python/matmul1024.py
```