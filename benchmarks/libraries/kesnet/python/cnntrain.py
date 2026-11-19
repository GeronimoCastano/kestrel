"""CNN training benchmark: Conv2d(1,8,3,padding=1) -> ReLU -> MaxPool2d(2)
-> Flatten -> Linear(1568,10), Adam, MSE against a fixed target."""
import torch
import torch.nn as nn
import torch.nn.functional as F


def data(shape, fn):
    grids = torch.meshgrid(
        *[torch.arange(n, dtype=torch.float64) for n in shape], indexing="ij"
    )
    return fn(*grids)


torch.set_default_dtype(torch.float64)
torch.manual_seed(2)

images = data(
    (32, 1, 28, 28),
    lambda n, c, i, j: ((n * 31 + i * 17 + j * 13) % 101 - 50.0) / 50.0,
)
targets = data((32, 10), lambda i, j: ((i * 43 + j * 19) % 89 - 44.0) / 44.0)

model = nn.Sequential(
    nn.Conv2d(1, 8, 3, padding=1),
    nn.ReLU(),
    nn.MaxPool2d(2),
    nn.Flatten(),
    nn.Linear(8 * 14 * 14, 10),
).double()
optimizer = torch.optim.Adam(model.parameters(), lr=0.001)

for step in range(950):
    optimizer.zero_grad()
    loss = F.mse_loss(model(images), targets)
    loss.backward()
    optimizer.step()

print(f"{loss.item():.6f}")
