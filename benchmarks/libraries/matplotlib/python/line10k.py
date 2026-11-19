"""line10k: two 5,000-point line series, title, labels, legend and grid."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_line10k.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    n = 5000
    x = 5.0 * np.arange(n) / (n - 1)
    fig, ax = plt.subplots(figsize=(8, 6))
    ax.plot(x, np.sin(x), label="sin(x)")
    ax.plot(x, np.cos(x), label="cos(x)", linestyle="--")
    ax.set_title("Two 5,000-point series")
    ax.set_xlabel("x")
    ax.set_ylabel("y")
    ax.legend(loc="upper right")
    ax.grid(True, color="#b0b0b0", linewidth=0.8, alpha=0.6)
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
