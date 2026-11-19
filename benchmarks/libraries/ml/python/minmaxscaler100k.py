"""MinMaxScaler fit and transform over 100,000 observations of 20 features,
repeated R times; the printed numbers sum the per-repetition checksums."""
import numpy as np
from sklearn.preprocessing import MinMaxScaler

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 300

rows, columns = 100_000, 20
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = ((i * 13 + j * 7) % 100) / 10.0
maximums = X.max(axis=0)

total_minimums = 0.0
total_maximums = 0.0
total_scales = 0.0
total_transformed = 0.0
for _ in range(REPETITIONS):
    scaler = MinMaxScaler()
    transformed = scaler.fit_transform(X)
    total_minimums += scaler.data_min_.sum()
    total_maximums += maximums.sum()
    total_scales += scaler.scale_.sum()
    total_transformed += transformed.sum()

print(
    f"{total_minimums:.6f} {total_maximums:.6f} "
    f"{total_scales:.6f} {total_transformed:.6f}"
)
