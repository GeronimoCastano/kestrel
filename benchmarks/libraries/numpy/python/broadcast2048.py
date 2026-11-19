import numpy as np

n = 2048
reps = 490
A = np.full((n, n), 1.5)
v = (np.arange(n) % 13).astype(np.float64)
vb = np.broadcast_to(v, (n, n))
checksum = 0.0
for _ in range(reps):
    result = (A + vb) * 0.5
    checksum += result.sum()
print(f"{checksum:.6f}")
