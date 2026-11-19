#!/usr/bin/env python3
"""Render Matplotlib counterparts for the Kestrel comparison cases.

This is an inspection tool, not a strict pixel test. It builds side-by-side
images, reports simple image-quality checks (blank/clipped output, ink ratio,
image aspect) and writes an HTML page for visual review.
"""

from __future__ import annotations

import html
import json
import math
import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import rcParams  # noqa: E402

OUT = Path("/tmp/kesplot-compare")
ROOT = Path(__file__).resolve().parents[1]
DPI = 150

CASES = [
    "line", "scatter", "bars", "barh", "hist", "box", "errors", "fill",
    "pie", "heatmap", "contour", "log", "math", "panels", "annotations",
    "inverted",
]

rcParams.update({
    "font.family": "serif",
    "font.serif": ["Times New Roman", "Times", "DejaVu Serif"],
    "mathtext.fontset": "stix",
    "text.usetex": True,
    "text.latex.preamble": r"\usepackage{newtxtext,newtxmath}",
    "axes.linewidth": 0.6,
    "axes.edgecolor": "black",
    "axes.labelcolor": "black",
    "xtick.color": "black",
    "ytick.color": "black",
    "xtick.direction": "in",
    "ytick.direction": "in",
    "xtick.top": True,
    "ytick.right": True,
    "xtick.major.size": 3,
    "ytick.major.size": 3,
    "xtick.minor.visible": True,
    "ytick.minor.visible": True,
    "xtick.minor.size": 1.5,
    "ytick.minor.size": 1.5,
    "legend.frameon": False,
    "figure.facecolor": "white",
})

SERIES = ["#0C5DA5", "#00B945", "#FF9500", "#FF2C00", "#845B97"]


def size_for(name: str) -> tuple[float, float]:
    ours = Image.open(OUT / f"kesplot-{name}.png")
    return ours.width / DPI, ours.height / DPI


def new_figure(name: str):
    width, height = size_for(name)
    return plt.figure(figsize=(width, height), dpi=DPI)


def finish(fig, name: str):
    fig.tight_layout()
    fig.savefig(OUT / f"mpl-{name}.png", dpi=DPI, facecolor="white")
    plt.close(fig)


def case_line():
    x = np.arange(101) / 20.0
    fig = new_figure("line")
    ax = fig.gca()
    ax.plot(x, np.sin(x), label="sin", color=SERIES[0], marker="o", ms=3, lw=1)
    ax.plot(x, np.cos(x), label="cos", color=SERIES[1], ls="--", lw=1)
    ax.set_title("Line plot")
    ax.set_xlabel("x")
    ax.set_ylabel("value")
    ax.legend(loc="upper right")
    ax.grid(True, alpha=0.3)
    finish(fig, "line")


def case_scatter():
    n = 45
    x = np.arange(n) * 0.9
    y = np.sin(np.arange(n) / 5.0) * 10.0 + np.arange(n) * 0.4
    s = 20.0 + np.arange(n) * 2.0
    fig = new_figure("scatter")
    ax = fig.gca()
    ax.scatter(x, y, s=s, label="points", color=SERIES[0], alpha=0.8)
    ax.set_title("Scatter")
    ax.set_xlabel("x")
    ax.set_ylabel("y")
    ax.legend(loc="upper left")
    finish(fig, "scatter")


def case_bars():
    cats = ["alpha", "beta", "gamma", "delta"]
    north = np.array([12.0, 8.0, 15.0, 10.0])
    south = np.array([9.0, 14.0, 7.0, 12.0])
    x = np.arange(len(cats))
    fig = new_figure("bars")
    ax = fig.gca()
    ax.bar(x - 0.18, north, width=0.36, label="north", color=SERIES[0])
    ax.bar(x + 0.18, south, width=0.36, label="south", color=SERIES[1])
    ax.set_xticks(x, cats)
    ax.set_title("Grouped bars")
    ax.set_xlabel("category")
    ax.set_ylabel("count")
    ax.legend(loc="upper right")
    finish(fig, "bars")


def case_barh():
    cats = ["north", "south", "east", "west"]
    values = [12.0, -4.0, 9.0, 6.0]
    fig = new_figure("barh")
    ax = fig.gca()
    ax.barh(cats, values, label="profit", color=SERIES[0])
    ax.set_title("Horizontal bars")
    ax.legend(loc="lower right")
    finish(fig, "barh")


def case_hist():
    a = np.sin(np.arange(200) * 0.7) * 2.0 + 0.5 * np.cos(np.arange(200) * 0.2)
    b = np.cos(np.arange(200) * 0.45) * 1.5 - 0.3
    fig = new_figure("hist")
    ax = fig.gca()
    ax.hist(a, bins=18, label="A", alpha=0.55, color=SERIES[0])
    ax.hist(b, bins=18, label="B", alpha=0.55, color=SERIES[1], histtype="step", lw=1.2)
    ax.set_title("Histograms")
    ax.set_xlabel("value")
    ax.set_ylabel("count")
    ax.legend(loc="upper right")
    finish(fig, "hist")


def case_box():
    data = [
        [1.0, 2.0, 2.2, 3.0, 4.0, 10.0, 2.5, 2.8],
        [2.0, 3.0, 3.4, 4.0, 5.0, 6.0, 3.2, 3.8],
        [0.5, 1.0, 1.1, 1.5, 2.0, 4.5, 1.2, 1.4],
    ]
    fig = new_figure("box")
    ax = fig.gca()
    bp = ax.boxplot(data, patch_artist=True, widths=0.5)
    for i, patch in enumerate(bp["boxes"]):
        patch.set_facecolor(SERIES[i % len(SERIES)])
        patch.set_alpha(0.45)
        patch.set_edgecolor(SERIES[i % len(SERIES)])
    ax.set_xticks([1, 2, 3], ["A", "B", "C"])
    ax.set_title("Box plots")
    ax.set_ylabel("value")
    finish(fig, "box")


def case_errors():
    x = np.arange(5.0)
    y = np.array([1.0, 2.5, 2.0, 3.5, 3.0])
    err = np.array([0.3, 0.5, 0.2, 0.6, 0.4])
    fig = new_figure("errors")
    ax = fig.gca()
    ax.errorbar(x, y, yerr=err, label="measurement", fmt="o-", capsize=4, color=SERIES[0])
    ax.set_title("Error bars")
    ax.set_xlabel("x")
    ax.set_ylabel("y")
    ax.legend(loc="best")
    finish(fig, "errors")


def case_fill():
    x = np.arange(61) / 10.0
    mean = np.sin(x)
    fig = new_figure("fill")
    ax = fig.gca()
    ax.fill_between(x, mean - 0.2, mean + 0.2, label="band", alpha=0.25, color=SERIES[0])
    ax.plot(x, mean, label="mean", color=SERIES[0])
    ax.set_title("Filled band")
    ax.set_xlabel("x")
    ax.legend(loc="upper right")
    finish(fig, "fill")


def case_pie():
    fig = new_figure("pie")
    ax = fig.gca()
    wedges, _, autotexts = ax.pie(
        [35, 25, 20, 12, 8],
        labels=["North", "South", "East", "West", "Other"],
        startangle=90,
        autopct="%1.1f%%",
        colors=SERIES + ["#9E9E9E"],
        wedgeprops={"width": 0.6, "edgecolor": "white"},
        textprops={"fontsize": 7},
    )
    ax.set_title("Share of sales")
    ax.legend(wedges, ["North", "South", "East", "West", "Other"], loc="center left", bbox_to_anchor=(1.0, 0.5))
    finish(fig, "pie")


def case_heatmap():
    r = np.arange(7)
    c = np.arange(6)
    matrix = np.sin(r[:, None] * 0.8) * np.cos(c[None, :] * 0.6)
    fig = new_figure("heatmap")
    ax = fig.gca()
    im = ax.imshow(matrix, cmap="viridis")
    ax.set_xticks(c, [f"c{i}" for i in c])
    ax.set_yticks(r, [f"r{i}" for i in r])
    ax.set_title("Heatmap")
    fig.colorbar(im, ax=ax, label="value")
    finish(fig, "heatmap")


def case_contour():
    r = np.arange(24)
    c = np.arange(40)
    matrix = np.sin(r[:, None] / 3.0) * np.cos(c[None, :] / 4.0)
    fig = new_figure("contour")
    ax = fig.gca()
    cs = ax.contour(matrix, levels=7, cmap="viridis")
    ax.set_title("Contour lines")
    fig.colorbar(cs, ax=ax, label="z")
    finish(fig, "contour")


def case_log():
    x = 1.0 + np.arange(40) * 2.0
    y = 10.0 * (1.15 ** np.arange(40))
    fig = new_figure("log")
    ax = fig.gca()
    ax.plot(x, y, label="growth", color=SERIES[0], marker="o", ms=3)
    ax.set_yscale("log")
    ax.set_title("Log y")
    ax.set_xlabel("x")
    ax.set_ylabel("count")
    ax.legend(loc="upper left")
    ax.grid(True, alpha=0.3)
    finish(fig, "log")


def case_math():
    x = np.arange(101) / 20.0
    fig = new_figure("math")
    ax = fig.gca()
    ax.plot(x, np.sin(x), label=r"$\sin(\pi x)$", color=SERIES[0])
    ax.plot(x, np.cos(x), label=r"$\cos(\pi x)$", ls="--", color=SERIES[1])
    ax.set_title(r"TeX: $\frac{a}{b} + x^2$")
    ax.set_xlabel(r"$\theta$ (rad)")
    ax.set_ylabel(r"$\sqrt{y}$")
    ax.legend(loc="upper right")
    finish(fig, "math")


def case_panels():
    x = np.arange(30)
    fig = new_figure("panels")
    axes = fig.subplots(2, 2)
    axes[0][0].plot(x, x * x, label="square")
    axes[0][0].set_title("Line")
    axes[0][0].legend()
    axes[0][1].scatter(x, np.sin(x / 3.0), label="scatter")
    axes[0][1].set_title("Scatter")
    axes[1][0].bar(["A", "B", "C", "D"], [3, 7, 2, 5], label="counts")
    axes[1][0].set_title("Bars")
    axes[1][1].hist(np.sin(np.arange(100) * 0.6), bins=12, label="hist")
    axes[1][1].set_title("Histogram")
    fig.suptitle("Panels", fontweight="bold")
    finish(fig, "panels")


def case_annotations():
    x = np.arange(61) / 10.0
    fig = new_figure("annotations")
    ax = fig.gca()
    ax.plot(x, np.sin(x), label="signal", color=SERIES[0])
    ax.axhline(0, label="zero", color=SERIES[3], ls="--")
    ax.annotate("peak", xy=(1.57, 1.0), xytext=(3.0, 0.7), color=SERIES[3],
                arrowprops={"arrowstyle": "->", "color": SERIES[3]})
    ax.set_title("Annotations")
    ax.set_xlabel("x")
    ax.legend(loc="lower left")
    finish(fig, "annotations")


def case_inverted():
    x = np.arange(51) / 10.0
    fig = new_figure("inverted")
    ax = fig.gca()
    ax.plot(x, np.sin(x), label="signal", color=SERIES[0])
    ax.set_xlim(5, 0)
    ax.set_title("Inverted x")
    ax.set_xlabel("x")
    ax.legend(loc="upper right")
    finish(fig, "inverted")


def build_references():
    case_line(); case_scatter(); case_bars(); case_barh(); case_hist()
    case_box(); case_errors(); case_fill(); case_pie(); case_heatmap()
    case_contour(); case_log(); case_math(); case_panels(); case_annotations()
    case_inverted()


def load_rgb(path: Path) -> Image.Image:
    image = Image.open(path).convert("RGB")
    return image


def metrics(path: Path) -> dict:
    image = load_rgb(path)
    array = np.asarray(image).astype(np.int16)
    gray = array.mean(axis=2)
    ink = gray < 248
    height, width = gray.shape
    border = np.zeros_like(ink)
    border[:2, :] = True
    border[-2:, :] = True
    border[:, :2] = True
    border[:, -2:] = True
    return {
        "width": width,
        "height": height,
        "ink": round(float(ink.mean()), 4),
        "border_ink": round(float(ink[border].mean()), 4),
        "colors": int(len(np.unique(array.reshape(-1, 3), axis=0))),
    }


def pair_image(name: str) -> Path:
    ours = load_rgb(OUT / f"kesplot-{name}.png")
    ref = load_rgb(OUT / f"mpl-{name}.png")
    target_h = 430
    scale = target_h / max(ours.height, ref.height)
    ours = ours.resize((max(1, int(ours.width * scale)), max(1, int(ours.height * scale))), Image.LANCZOS)
    ref = ref.resize((max(1, int(ref.width * scale)), max(1, int(ref.height * scale))), Image.LANCZOS)
    gap = 18
    label_h = 28
    width = ours.width + ref.width + gap
    height = max(ours.height, ref.height) + label_h
    canvas = Image.new("RGB", (width, height), "white")
    canvas.paste(ours, (0, label_h))
    canvas.paste(ref, (ours.width + gap, label_h))
    draw = ImageDraw.Draw(canvas)
    try:
        font = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 16)
    except Exception:
        font = ImageFont.load_default()
    draw.text((4, 5), f"kesplot: {name}", fill="#0C5DA5", font=font)
    draw.text((ours.width + gap + 4, 5), f"matplotlib: {name}", fill="#444444", font=font)
    path = OUT / f"pair-{name}.png"
    canvas.save(path)
    return path


def build_html(results: dict):
    rows = []
    for name in CASES:
        ours = results[name]["ours"]
        ref = results[name]["reference"]
        rows.append(
            f'<section><h2>{html.escape(name)}</h2>'
            f'<img src="pair-{name}.png" alt="{name} comparison">'
            f'<p>kesplot {ours["width"]}×{ours["height"]}, ink {ours["ink"]:.3f}, '
            f'border ink {ours["border_ink"]:.4f}; Matplotlib {ref["width"]}×{ref["height"]}, '
            f'ink {ref["ink"]:.3f}.</p></section>'
        )
    page = '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>kesplot vs Matplotlib</title>
<style>
body{font-family:-apple-system,Segoe UI,Arial,sans-serif;background:#fafafa;color:#222;margin:2rem auto;max-width:1200px}
h1{font-weight:600} h2{font-weight:600;font-size:1.05rem;margin:1.8rem 0 .4rem}
section{background:white;border:1px solid #e2e2e2;border-radius:6px;padding:1rem;margin:1rem 0}
img{max-width:100%;height:auto;display:block} p{color:#555;font-size:.9rem}
code{background:#eee;padding:1px 4px;border-radius:3px}
</style></head><body>
<h1>kesplot vs Matplotlib</h1>
<p>Matplotlib is a reference for geometry and data placement, not a pixel target. Both figures use
the same data and both use TeX for math labels (Matplotlib through text.usetex with the newtx
fonts). Matplotlib is a geometry reference, not a pixel target. Look for missing data, clipped
labels, wrong axis directions, broken legends and wrong scales.</p>
''' + "\n".join(rows) + "\n</body></html>\n"
    (OUT / "comparison.html").write_text(page)


def main() -> int:
    if not shutil.which("rsvg-convert"):
        print("rsvg-convert is required", file=sys.stderr)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [str(ROOT / "target/debug/kestrel"), "run", str(ROOT / "tools/kesplotcases.kes")],
        cwd=ROOT, check=True,
    )
    build_references()

    results = {}
    failures = []
    for name in CASES:
        ours_path = OUT / f"kesplot-{name}.png"
        ref_path = OUT / f"mpl-{name}.png"
        if not ours_path.exists():
            failures.append(f"{name}: kesplot image missing")
            continue
        if not ref_path.exists():
            failures.append(f"{name}: matplotlib image missing")
            continue
        ours = metrics(ours_path)
        ref = metrics(ref_path)
        results[name] = {"ours": ours, "reference": ref}
        pair_image(name)
        if ours["ink"] < 0.004:
            failures.append(f"{name}: kesplot output is nearly blank (ink {ours['ink']})")
        if ours["border_ink"] > 0.02:
            # Use a stricter darkness threshold so a half-covered antialiased
            # edge row from the rasterizer does not look like clipped content.
            image = np.asarray(load_rgb(ours_path)).astype(np.int16)
            dark = image.mean(axis=2) < 100
            edge = np.zeros_like(dark)
            edge[:2, :] = True
            edge[-2:, :] = True
            edge[:, :2] = True
            edge[:, -2:] = True
            if dark[edge].mean() > 0.01:
                failures.append(f"{name}: kesplot ink touches the image border ({ours['border_ink']})")
        if ours["width"] < 200 or ours["height"] < 150:
            failures.append(f"{name}: kesplot output is too small")

    build_html(results)
    pairs = sorted(OUT.glob("pair-*.png"))
    if pairs:
        thumb_w = 640
        thumbs = []
        for path in pairs:
            image = load_rgb(path)
            scale = thumb_w / image.width
            thumbs.append(image.resize((thumb_w, int(image.height * scale)), Image.LANCZOS))
        cols = 2
        rows = math.ceil(len(thumbs) / cols)
        cell_h = max(image.height for image in thumbs) + 8
        sheet = Image.new("RGB", (cols * thumb_w + (cols + 1) * 8, rows * cell_h + (rows + 1) * 8), "#e8e8e8")
        for index, image in enumerate(thumbs):
            col = index % cols
            row = index // cols
            sheet.paste(image, (8 + col * (thumb_w + 8), 8 + row * cell_h))
        sheet.save(OUT / "contact.png")

    (OUT / "metrics.json").write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))
    print(f"\nHTML: {OUT / 'comparison.html'}")
    print(f"contact sheet: {OUT / 'contact.png'}")
    if failures:
        print("\nFAILURES:", file=sys.stderr)
        for failure in failures:
            print(" -", failure, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
