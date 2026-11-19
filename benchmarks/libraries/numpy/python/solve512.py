import numpy as np

n = 512
reps = 1280
A = np.full((n, n), 0.01)
np.fill_diagonal(A, 4.0)
b = (np.arange(n) % 7).astype(np.float64)
total = 0.0
norm_total = 0.0
for _ in range(reps):
    x = np.linalg.solve(A, b)
    total += x.sum()
    norm_total += np.linalg.norm(x)
print(f"{total:.6f}")
print(f"{norm_total:.6f}")
