"""hist1m: histogram of 1,000,000 deterministic values in [0, 100), 50 bins."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_hist1m.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    n = 1000000
    i = np.arange(n, dtype=np.int64)
    hashed = (i * 2654435761 + 12345) % 2147483648
    values = 100.0 * hashed / 2147483648.0
    fig, ax = plt.subplots(figsize=(8, 6))
    ax.hist(values, bins=50, color="C0")
    ax.set_title("Histogram of 1,000,000 values")
    ax.set_xlabel("value")
    ax.set_ylabel("count")
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
