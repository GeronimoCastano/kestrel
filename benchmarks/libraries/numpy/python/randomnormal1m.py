import numpy as np

reps = 300
rng = np.random.default_rng(42)
total = 0.0
for _ in range(reps):
    total += rng.standard_normal(1_000_000).sum()
print(f"{total:.6f}")
