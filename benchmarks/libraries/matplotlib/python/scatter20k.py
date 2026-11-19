"""scatter20k: 20,000 scatter points with per-point sizes."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_scatter20k.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    n = 20000
    i = np.arange(n, dtype=np.float64)
    u = np.mod(np.sin(i * 12.9898 + 78.233) * 43758.5453, 1.0)
    v = np.mod(np.sin(i * 39.3467 + 11.135) * 24634.6345, 1.0)
    x = 10.0 * u
    y = 10.0 * v
    sizes = 2.0 + (np.arange(n) % 10) * 2.0
    fig, ax = plt.subplots(figsize=(8, 6))
    ax.scatter(x, y, s=sizes, marker="o", alpha=0.8, edgecolors="none", label="points")
    ax.set_title("20,000 scatter points")
    ax.set_xlabel("x")
    ax.set_ylabel("y")
    ax.legend(loc="upper right")
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
