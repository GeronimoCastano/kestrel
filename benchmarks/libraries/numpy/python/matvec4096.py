import numpy as np

n = 4096
reps = 690
A = np.full((n, n), 1.5)
x = (np.arange(n) % 97).astype(np.float64)
checksum = 0.0
for _ in range(reps):
    y = A @ x
    checksum += y.sum()
print(f"{checksum:.6f}")
