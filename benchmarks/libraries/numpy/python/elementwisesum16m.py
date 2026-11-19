import numpy as np

n = 16000000
reps = 23
a = np.full(n, 1.5)
b = np.full(n, 0.5)
c = np.full(n, 2.0)
checksum = 0.0
for _ in range(reps):
    result = (a + b) * c - a / c + np.sqrt(a)
    checksum += result.sum()
print(f"{checksum:.6f}")
