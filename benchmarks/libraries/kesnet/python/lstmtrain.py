"""LSTM training benchmark: sequence 16, batch 16, input 32, hidden 64; the last
hidden state is projected to one output; Adam, MSE."""
import torch
import torch.nn as nn
import torch.nn.functional as F


def data(shape, fn):
    grids = torch.meshgrid(
        *[torch.arange(n, dtype=torch.float64) for n in shape], indexing="ij"
    )
    return fn(*grids)


torch.set_default_dtype(torch.float64)
torch.manual_seed(4)

sequence = data(
    (16, 16, 32),
    lambda t, n, f: ((t * 41 + n * 23 + f * 13) % 101 - 50.0) / 50.0,
)
targets = data((16, 1), lambda i, j: ((i * 37 + j * 11) % 89 - 44.0) / 44.0)

model = nn.LSTM(32, 64).double()
head = nn.Linear(64, 1).double()
optimizer = torch.optim.Adam([*model.parameters(), *head.parameters()], lr=0.005)

for step in range(680):
    optimizer.zero_grad()
    output, _ = model(sequence)
    loss = F.mse_loss(head(output[-1]), targets)
    loss.backward()
    optimizer.step()

print(f"{loss.item():.6f}")
