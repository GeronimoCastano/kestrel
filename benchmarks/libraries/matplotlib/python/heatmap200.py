"""heatmap200: a 200x200 matrix drawn with a vector pcolormesh plus a colorbar."""
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

plt.rcParams["path.simplify"] = False

OUT = "/tmp/plotbench/python_heatmap200.svg"


def main():
    os.makedirs("/tmp/plotbench", exist_ok=True)
    r = np.arange(200, dtype=np.float64)[:, None]
    c = np.arange(200, dtype=np.float64)[None, :]
    matrix = 0.5 + 0.5 * np.sin(r * 0.05) * np.cos(c * 0.05)
    fig, ax = plt.subplots(figsize=(8, 6))
    mesh = ax.pcolormesh(
        np.arange(201), np.arange(201), matrix[::-1], cmap="viridis", shading="flat"
    )
    mesh.set_rasterized(False)
    colorbar = fig.colorbar(mesh, ax=ax, label="value")
    colorbar.solids.set_rasterized(False)
    ax.set_title("200 x 200 heatmap")
    fig.savefig(OUT)
    print(os.path.getsize(OUT))


main()
