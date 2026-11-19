import numpy as np

n = 2000000
reps = 22
x = (np.arange(n) % 1000).astype(np.float64)
y = ((np.arange(n) * 7) % 1000).astype(np.float64)
data = np.column_stack((x, y))
mean_total = 0.0
variance_total = 0.0
stddev_total = 0.0
covariance_total = 0.0
correlation_total = 0.0
for _ in range(reps):
    mean_total += np.mean(x)
    variance_total += np.var(x)
    stddev_total += np.std(x)
    covariance_total += np.cov(data, rowvar=False, ddof=0)[0, 1]
    correlation_total += np.corrcoef(data, rowvar=False)[0, 1]
print(f"{mean_total:.6f}")
print(f"{variance_total:.6f}")
print(f"{stddev_total:.6f}")
print(f"{covariance_total:.6f}")
print(f"{correlation_total:.6f}")
