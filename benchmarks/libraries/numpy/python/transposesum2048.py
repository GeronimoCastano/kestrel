import numpy as np

n = 2048
reps = 190
A = ((np.arange(n)[:, None] * 7 + np.arange(n)[None, :] * 11) % 97).astype(np.float64)
checksum = 0.0
slice_total = 0.0
for _ in range(reps):
    checksum += A.T.sum()
    slice_total += A[0:1024, 0:1024].sum()
print(f"{checksum:.6f}")
print(f"{slice_total:.6f}")
