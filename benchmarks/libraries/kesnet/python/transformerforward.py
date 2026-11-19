"""Transformer forward/backward benchmark: MultiheadAttention(64, 2) -> residual
-> LayerNorm -> Linear(64, 64). Backward on the output sum, 1600 iterations."""
import torch
import torch.nn as nn


def data(shape, fn):
    grids = torch.meshgrid(
        *[torch.arange(n, dtype=torch.float64) for n in shape], indexing="ij"
    )
    return fn(*grids)


torch.set_default_dtype(torch.float64)
torch.manual_seed(3)


class AttentionBlock(nn.Module):
    def __init__(self, width, heads):
        super().__init__()
        self.attention = nn.MultiheadAttention(width, heads, batch_first=True)
        self.norm = nn.LayerNorm(width)
        self.projection = nn.Linear(width, width)

    def forward(self, x):
        attended, _ = self.attention(x, x, x)
        return self.projection(self.norm(attended + x))


tokens = data(
    (8, 32, 64),
    lambda n, t, f: ((n * 29 + t * 47 + f * 11) % 101 - 50.0) / 50.0,
)

block = AttentionBlock(64, 2).double()

for step in range(1600):
    block.zero_grad()
    loss = block(tokens).sum()
    loss.backward()

print(f"{loss.item():.6f}")
