"""Train/test split of 50,000 rows with 6 features and y as the row sums.

The split repeats R times. Kestrel's Rng and scikit-learn's shuffle produce
different permutations, so the checksum is the split-invariant column sums of X
over both outputs, added across repetitions.
"""
import numpy as np
from sklearn.model_selection import train_test_split

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 1500

rows, columns = 50_000, 6
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = ((i * 1103515245 + j * 12345 + 7) % 2147483648) / 2147483648.0
y = X.sum(axis=1)

total = np.zeros(columns)
for _ in range(REPETITIONS):
    trainingX, testingX, trainingY, testingY = train_test_split(
        X, y, test_size=10_000, random_state=7
    )
    total += trainingX.sum(axis=0) + testingX.sum(axis=0)

print(" ".join(f"{value:.6f}" for value in total))
