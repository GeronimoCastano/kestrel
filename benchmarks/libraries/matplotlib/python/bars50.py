"""bars50: 50 groups, two grouped bar series, title and legend."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_bars50.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    n = 50
    i = np.arange(n)
    north = 10 + ((i * 7) % 13)
    south = 8 + ((i * 5) % 11)
    fig, ax = plt.subplots(figsize=(8, 6))
    ax.bar(i - 0.2, north, width=0.4, label="north")
    ax.bar(i + 0.2, south, width=0.4, label="south")
    ax.set_title("Grouped bars: 50 groups, 2 series")
    ax.set_xlabel("group")
    ax.set_ylabel("value")
    ax.legend(loc="upper right")
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
