"""StandardScaler fit and transform over 100,000 observations of 20 features,
repeated R times; the printed numbers sum the per-repetition checksums.

The transformed sum is rounded to the six printed decimals per repetition so
the near-zero values add up consistently.
"""
import numpy as np
from sklearn.preprocessing import StandardScaler

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 300

rows, columns = 100_000, 20
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = ((i * 13 + j * 7) % 100) / 10.0

total_means = 0.0
total_scales = 0.0
total_transformed = 0.0
for _ in range(REPETITIONS):
    scaler = StandardScaler()
    transformed = scaler.fit_transform(X)
    total_means += scaler.mean_.sum()
    total_scales += scaler.scale_.sum()
    total_transformed += round(transformed.sum(), 6)

print(f"{total_means:.6f} {total_scales:.6f} {total_transformed:.6f}")
