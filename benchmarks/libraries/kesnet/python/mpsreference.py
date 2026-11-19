#!/usr/bin/env python3
"""PyTorch CPU versus PyTorch MPS reference, for context only.

Kestrel has no MPS backend yet: its tensor storage and every kernel are
double precision, and Apple GPUs do not implement fp64 in Metal. PyTorch's MPS
path is float32-only, so this script compares PyTorch to itself on the two
devices and gives the reader the hardware reference point. It is not part of
the Kestrel comparison and the library runner does not pick it up.

    python3 benchmarks/libraries/kesnet/python/mpsreference.py
"""

from __future__ import annotations

import time

import torch
import torch.nn as nn
import torch.nn.functional as F


def timed(device: str, repeats: int, model: nn.Module, batches, loss_fn) -> float:
    model = model.to(device)
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
    for _ in range(2):
        for (x, y) in batches:
            x, y = x.to(device), y.to(device)
            optimizer.zero_grad()
            loss = loss_fn(model(x), y)
            loss.backward()
            optimizer.step()
    if device == "mps":
        torch.mps.synchronize()
    start = time.perf_counter()
    for _ in range(repeats):
        for (x, y) in batches:
            x, y = x.to(device), y.to(device)
            optimizer.zero_grad()
            loss = loss_fn(model(x), y)
            loss.backward()
            optimizer.step()
    if device == "mps":
        torch.mps.synchronize()
    return (time.perf_counter() - start) / (repeats * len(batches))


def make_batches(dtype: torch.dtype, count: int):
    g = torch.Generator().manual_seed(11)
    batches = []
    for _ in range(count):
        x = torch.randn(128, 784, generator=g, dtype=torch.float32).to(dtype)
        y = torch.randn(128, 10, generator=g, dtype=torch.float32).to(dtype)
        batches.append((x, y))
    return batches


def mlp(dtype: torch.dtype) -> nn.Module:
    return nn.Sequential(nn.Linear(784, 256), nn.ReLU(), nn.Linear(256, 10)).to(dtype)


def transformer_step(dtype: torch.dtype, batch: int, seq: int, width: int, heads: int) -> nn.Module:
    return nn.TransformerEncoderLayer(
        d_model=width, nhead=heads, dim_feedforward=width * 4,
        batch_first=True, dtype=dtype,
    )


def main() -> None:
    print(f"torch {torch.__version__}  mps available: {torch.backends.mps.is_available()}")
    repeats, batch_count = 20, 10
    for device in ("cpu", "mps"):
        if device == "mps" and not torch.backends.mps.is_available():
            continue
        batches = make_batches(torch.float32, batch_count)
        model = mlp(torch.float32)
        seconds = timed(device, repeats, model, batches, F.mse_loss)
        print(f"mlp       {device}: {seconds * 1000:8.2f} ms per step")

    width, heads, seq, batch = 64, 2, 32, 8
    for device in ("cpu", "mps"):
        if device == "mps" and not torch.backends.mps.is_available():
            continue
        g = torch.Generator().manual_seed(7)
        x = torch.randn(batch, seq, width, generator=g).to(device)
        layer = transformer_step(torch.float32, batch, seq, width, heads).to(device)
        optimizer = torch.optim.Adam(layer.parameters(), lr=1e-3)
        for _ in range(10):
            optimizer.zero_grad()
            out = layer(x)
            loss = out.sum()
            loss.backward()
            optimizer.step()
        if device == "mps":
            torch.mps.synchronize()
        start = time.perf_counter()
        for _ in range(100):
            optimizer.zero_grad()
            out = layer(x)
            loss = out.sum()
            loss.backward()
            optimizer.step()
        if device == "mps":
            torch.mps.synchronize()
        print(f"transform {device}: {(time.perf_counter() - start) * 10:8.2f} ms per step")


if __name__ == "__main__":
    main()
