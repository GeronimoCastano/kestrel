"""Logistic regression on 20,000 observations, 10 features.

The fit and prediction repeat R times and the printed accuracy adds every
repetition. Kestrel uses batch gradient descent while scikit-learn uses L-BFGS,
so only training accuracy is compared.
"""
import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import accuracy_score

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 150

rows, columns = 20_000, 10
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]
X = (((i * 1103515245 + j * 12345 + 7) % 2001) - 1000) / 1000.0

score = X[:, 0] + 0.8 * X[:, 1] - 0.6 * X[:, 2] + 0.4 * X[:, 3]
labels = (score > 0.0).astype(np.float64)
flip = np.arange(rows) % 41 == 0
labels[flip] = 1.0 - labels[flip]

total_accuracy = 0.0
for _ in range(REPETITIONS):
    model = LogisticRegression(max_iter=200)
    predicted = model.fit(X, labels).predict(X)
    total_accuracy += accuracy_score(labels, predicted)

print(f"{total_accuracy:.6f}")
