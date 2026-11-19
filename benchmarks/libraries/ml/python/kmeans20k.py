"""K-means with four clusters over 20,000 points from three deterministic blobs.

The fit and prediction repeat R times and the printed counts and label sum add
every repetition. Kestrel seeds from the first rows and scikit-learn uses
k-means++; the checksums are not comparable, so this workload is timing-only.
"""
import numpy as np
from sklearn.cluster import KMeans

import sys

REPETITIONS = int(sys.argv[1]) if len(sys.argv) > 1 else 400

rows, columns = 20_000, 12
i = np.arange(rows)[:, None]
j = np.arange(columns)[None, :]

cluster = i % 3
first = np.where(cluster == 0, -3.0, np.where(cluster == 1, 3.0, 0.0))
second = np.where(cluster == 0, -3.0, np.where(cluster == 1, -3.0, 3.0))
center = np.where(j == 0, first, np.where(j == 1, second, 0.0))
noise = ((i * 37 + j * 53 + 11) % 200 - 100.0) / 200.0
X = center + noise

total_counts = np.zeros(4, dtype=np.int64)
total_labels = 0
for _ in range(REPETITIONS):
    model = KMeans(n_clusters=4, n_init=1, max_iter=50, random_state=0)
    labels = model.fit_predict(X)
    total_counts += np.bincount(labels, minlength=4)
    total_labels += labels.sum()

print(
    " ".join(f"{value:.6f}" for value in np.sort(total_counts))
    + f" {total_labels:.6f}"
)
