import numpy as np

n = 4096
reps = 80
A = np.full((n, n), 1.5)
total = 0.0
mean_total = 0.0
min_total = 0.0
max_total = 0.0
axis_total = 0.0
for _ in range(reps):
    total += A.sum()
    mean_total += A.mean()
    min_total += A.min()
    max_total += A.max()
    axis_total += A.sum(axis=0).sum()
print(f"{total:.6f}")
print(f"{mean_total:.6f}")
print(f"{min_total:.6f}")
print(f"{max_total:.6f}")
print(f"{axis_total:.6f}")
