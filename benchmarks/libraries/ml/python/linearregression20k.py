"""Closed-form least squares on 20,000 observations with 16 features.

The fit and prediction repeat R times and the printed numbers add every
repetition.
"""
import numpy as np
from sklearn.linear_model import LinearRegression
from sklearn.metrics import mean_squared_error

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 500

rows, columns = 20_000, 16
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = (((i * 31 + j * 17) % 101) - 50) / 10.0
y = 3.0 * X[:, 0] - 2.0 * X[:, 1] + 0.5 * X[:, 2] + 1.0

total_intercept = 0.0
total_coefficients = 0.0
total_error = 0.0
for _ in range(REPETITIONS):
    model = LinearRegression()
    predicted = model.fit(X, y).predict(X)
    total_intercept += model.intercept_
    total_coefficients += model.coef_.sum()
    total_error += mean_squared_error(y, predicted)

print(
    f"{total_intercept:.6f} {total_coefficients:.6f} {total_error:.6f}"
)
