import numpy as np

n = 512
reps = 82
A = np.full((n, n), 0.01)
np.fill_diagonal(A, 2.0)
values_total = 0.0
residual_total = 0.0
for _ in range(reps):
    w, v = np.linalg.eigh(A)
    values_total += w.sum()
    residual_total += np.linalg.norm(A @ v - v @ np.diag(w))
print(f"{values_total:.6f}")
print(f"{residual_total:.6f}")
