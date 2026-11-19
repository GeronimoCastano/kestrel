"""panels4: a 2x2 grid of four 1,000-point line panels."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_panels4.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    n = 1000
    x = 10.0 * np.arange(n) / (n - 1)
    series = [
        (np.sin(x), "sin(x)"),
        (np.cos(x), "cos(x)"),
        (np.sin(2.0 * x), "sin(2x)"),
        (np.cos(2.0 * x), "cos(2x)"),
    ]
    fig, axes = plt.subplots(2, 2, figsize=(8, 6))
    for ax, (y, name) in zip(axes.flat, series):
        ax.plot(x, y, label=name)
        ax.set_title(name)
        ax.set_xlabel("x")
        ax.set_ylabel("y")
        ax.legend(loc="upper right")
    fig.suptitle("Four 1,000-point panels")
    fig.tight_layout(rect=(0, 0, 1, 0.95))
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
