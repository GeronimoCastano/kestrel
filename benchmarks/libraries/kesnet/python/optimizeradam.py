"""Adam/backward overhead benchmark: 784 -> 64 -> 10 MLP, batch 64, 3400 steps."""
import torch
import torch.nn as nn
import torch.nn.functional as F


def data(shape, fn):
    grids = torch.meshgrid(
        *[torch.arange(n, dtype=torch.float64) for n in shape], indexing="ij"
    )
    return fn(*grids)


torch.set_default_dtype(torch.float64)
torch.manual_seed(5)

features = data((64, 784), lambda i, j: ((i * 37 + j * 17) % 101 - 50.0) / 50.0)
targets = data((64, 10), lambda i, j: ((i * 53 + j * 29) % 97 - 48.0) / 48.0)

model = nn.Sequential(nn.Linear(784, 64), nn.ReLU(), nn.Linear(64, 10)).double()
optimizer = torch.optim.Adam(model.parameters(), lr=0.001)

for step in range(3400):
    optimizer.zero_grad()
    loss = F.mse_loss(model(features), targets)
    loss.backward()
    optimizer.step()

print(f"{loss.item():.6f}")
