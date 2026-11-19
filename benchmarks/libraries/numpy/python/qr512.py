import numpy as np

n = 512
reps = 140
A = np.full((n, n), 0.01)
np.fill_diagonal(A, 4.0)
identity = np.eye(n)
trace_total = 0.0
residual_total = 0.0
for _ in range(reps):
    q, r = np.linalg.qr(A)
    trace_total += np.trace(r)
    residual_total += np.linalg.norm(q.T @ q - identity)
print(f"{trace_total:.6f}")
print(f"{residual_total:.6f}")
