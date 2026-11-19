import numpy as np

n = 256
reps = 170
i = np.arange(n)[:, None]
j = np.arange(n)[None, :]
A = ((i * 13 + j * 7) % 89).astype(np.float64) / 10.0
singular_total = 0.0
residual_total = 0.0
for _ in range(reps):
    u, s, vt = np.linalg.svd(A)
    singular_total += s.sum()
    residual_total += np.linalg.norm(u @ np.diag(s) @ vt - A)
print(f"{singular_total:.6f}")
print(f"{residual_total:.6f}")
