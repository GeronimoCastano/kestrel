"""Binary classification metrics over 100,000 labels.

The metric computation repeats R times and the printed numbers add every
repetition.
"""
import numpy as np
from sklearn.metrics import (
    accuracy_score,
    confusion_matrix,
    f1_score,
    precision_score,
    recall_score,
)

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 80

rows = 100_000
i = np.arange(rows)
actual = ((((i * 1103515245 + 12345) % 2147483648) % 100) < 40).astype(np.float64)
predicted = np.where(i % 17 == 0, 1.0 - actual, actual)

total_accuracy = 0.0
total_precision = 0.0
total_recall = 0.0
total_f1 = 0.0
total_cells = np.zeros(4)
for _ in range(REPETITIONS):
    total_accuracy += accuracy_score(actual, predicted)
    total_precision += precision_score(actual, predicted)
    total_recall += recall_score(actual, predicted)
    total_f1 += f1_score(actual, predicted)
    total_cells += confusion_matrix(actual, predicted, labels=[0.0, 1.0]).ravel()

print(
    f"{total_accuracy:.6f} {total_precision:.6f} "
    f"{total_recall:.6f} {total_f1:.6f} "
    f"{total_cells[0]:.6f} {total_cells[1]:.6f} "
    f"{total_cells[2]:.6f} {total_cells[3]:.6f}"
)
