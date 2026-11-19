"""PCA with five components over 10,000 observations of 30 features.

The fit and transform repeat R times and the printed numbers add every
repetition.
"""
import numpy as np
from sklearn.decomposition import PCA

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 1500

rows, columns = 10_000, 30
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = ((i * 1103515245 + j * 12345 + 7) % 2147483648) / 2147483648.0 - 0.5

total_variance = 0.0
total_transformed = 0.0
for _ in range(REPETITIONS):
    model = PCA(n_components=5)
    transformed = model.fit_transform(X)
    total_variance += model.explained_variance_.sum()
    total_transformed += transformed.sum()

print(f"{total_variance:.6f} {total_transformed:.6f}")
