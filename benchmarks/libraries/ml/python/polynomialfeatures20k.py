"""Degree-two polynomial expansion of 20,000 observations with 10 features,
including the bias column, matching scikit-learn's include_bias=True.

The transform repeats R times and the printed sum adds every repetition.
"""
import numpy as np
from sklearn.preprocessing import PolynomialFeatures

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 800

rows, columns = 20_000, 10
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = ((i * 17 + j * 11) % 100) / 10.0

poly = PolynomialFeatures(degree=2, include_bias=True).fit(X)
total = 0.0
for _ in range(REPETITIONS):
    total += poly.transform(X).sum()

print(f"{poly.n_output_features_:.6f} {total:.6f}")
