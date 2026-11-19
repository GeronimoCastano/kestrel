"""K-nearest-neighbour prediction: 20,000 training rows, 2,000 test rows, 8 features.

The prediction repeats R times and the printed numbers add every repetition.
"""
import numpy as np
from sklearn.metrics import accuracy_score
from sklearn.neighbors import KNeighborsClassifier

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 10

train_rows, test_rows, columns = 20_000, 2_000, 8

i = np.arange(train_rows)[:, None]
j = np.arange(columns)[None, :]
training = ((i * 1103515245 + j * 12345 + 12345) % 2147483648) / 2147483648.0

i = np.arange(test_rows)[:, None]
testing = ((i * 1103515245 + j * 12345 + 987654321) % 2147483648) / 2147483648.0

labels = (training[:, 0] > 0.5).astype(np.float64)
expected = (testing[:, 0] > 0.5).astype(np.float64)

model = KNeighborsClassifier(n_neighbors=5).fit(training, labels)
total_accuracy = 0.0
total_predictions = 0.0
for _ in range(REPETITIONS):
    predicted = model.predict(testing)
    total_accuracy += accuracy_score(expected, predicted)
    total_predictions += predicted.sum()

print(f"{total_accuracy:.6f} {total_predictions:.6f}")
