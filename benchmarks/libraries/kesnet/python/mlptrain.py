"""MLP training benchmark: 784 -> 256 -> 10, Adam, MSE against a fixed target."""
import torch
import torch.nn as nn
import torch.nn.functional as F


def data(shape, fn):
    grids = torch.meshgrid(
        *[torch.arange(n, dtype=torch.float64) for n in shape], indexing="ij"
    )
    return fn(*grids)


torch.set_default_dtype(torch.float64)
torch.manual_seed(1)

features = data((128, 784), lambda i, j: ((i * 37 + j * 17) % 101 - 50.0) / 50.0)
targets = data((128, 10), lambda i, j: ((i * 53 + j * 29) % 97 - 48.0) / 48.0)

model = nn.Sequential(nn.Linear(784, 256), nn.ReLU(), nn.Linear(256, 10)).double()
optimizer = torch.optim.Adam(model.parameters(), lr=0.001)

for step in range(1000):
    optimizer.zero_grad()
    loss = F.mse_loss(model(features), targets)
    loss.backward()
    optimizer.step()

print(f"{loss.item():.6f}")
