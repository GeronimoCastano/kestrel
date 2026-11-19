import numpy as np

n = 1024
reps = 300
a = np.full((n, n), 1.5)
b = np.full((n, n), 0.5)
checksum = 0.0
for _ in range(reps):
    c = a @ b
    checksum += c.sum()
print(f"{checksum:.6f}")
