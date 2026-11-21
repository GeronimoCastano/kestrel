# std.kesnet

Deep learning in Kestrel: tensors that record operations, reverse-mode autograd,
layers, losses and optimizers, all native code.

```kestrel
import std.kesnet.{Linear, ReLU, Sequential, mseLoss}
import std.kesnet.optim.{Adam, StepLR}
import std.random

Variable.seed(11)
let inputs := Variable.fromTensor(Tensor([1.0, 2.0, 2.0, 1.0, 4.0, 3.0, 5.0, 6.0], shape = (4, 2)))
let expected := Variable.fromTensor(Tensor([8.0, 7.0, 18.0, 29.0], shape = (4, 1)))

let model := Sequential([box(Linear(2, 16)), box(ReLU()), box(Linear(16, 1))])
let mut optimizer := Adam(model.parameters(), 0.03)

for (epoch in range(200)) {
    let loss := mseLoss(model.forward(inputs), expected)
    optimizer.zeroGrad()
    loss.backward()
    optimizer.step()
    if epoch % 50 == 0 {
        print(f"epoch {epoch}: loss {loss.item()}")
    }
}
```

A model is a `Module`: a class with `forward`, and the parameters it registers.
`Sequential` runs its children in order. `box` wraps a value so a
`List[Module]` can hold different subclasses.

## Variables

| Call | Meaning |
|---|---|
| `Variable.fromTensor(t)` | A value that records the operations applied to it. |
| `Variable.seed(n)` | Fixes the random initialisation, so a run is reproducible. |
| `v.data()` | The tensor it holds. |
| `v.item()` | The single number in a one-element value. |
| `loss.backward()` | Fills every gradient by walking back through the graph. |
| `v.zeroGrad()` | Clears the gradients of a value and the ones it depends on. |

## Modules

| Method | Meaning |
|---|---|
| `module.forward(x)` | The forward pass. |
| `module.parameters()` | The `Variable`s that training updates. |
| `module.parameterCount()` | How many numbers that is. |
| `module.train()`, `module.eval()` | Switch dropout and normalisation to training or inference. |
| `module.zeroGrad()` | Clear every gradient under the module. |
| `module.summary()` | A line per child with its shapes. |

## Layers

| Group | Classes |
|---|---|
| Dense | `Linear(inFeatures, outFeatures)` |
| Activations | `ReLU`, `LeakyReLU`, `ELU`, `GELU`, `SiLU`, `Sigmoid`, `Tanh`, `Softmax`, `LogSoftmax` |
| Shape | `Flatten`, `Identity` |
| Regularisation | `Dropout(probability)` |
| Convolution | `Conv2d`, `MaxPool2d`, `AvgPool2d`, `AdaptiveAvgPool2d` |
| Normalisation | `BatchNorm1d`, `BatchNorm2d`, `LayerNorm`, `GroupNorm` |
| Sequence | `Sequential`, `ModuleList` |
| Attention | `Embedding`, `MultiheadAttention`, `TransformerEncoderLayer`, `TransformerEncoder` |
| Recurrent | `RNNCell`, `RNN`, `LSTMCell`, `LSTM`, `GRUCell`, `GRU` |

Every layer is a `Module`, so it can go into `Sequential`.

## Losses

| Function | Meaning |
|---|---|
| `mseLoss(prediction, target)` | Mean squared error. |
| `l1Loss(prediction, target)` | Mean absolute error. |
| `crossEntropy(logits, target)` | Softmax and negative log-likelihood together. |
| `nllLoss(logProbabilities, target)` | Negative log-likelihood of already normalised logits. |
| `binaryCrossEntropy(logits, target)` | Binary cross-entropy from logits. |

The classes `MSELoss`, `L1Loss`, `CrossEntropyLoss`, `NLLLoss` and
`BCEWithLogitsLoss` wrap the same functions. `meanSquaredError(a, b)` returns a
plain `double`, for reporting.

## Optimizers and schedules

| Class | Meaning |
|---|---|
| `SGD(parameters, learningRate)` | Plain gradient descent, with momentum when asked. |
| `Adam`, `AdamW` | Adaptive moments, with and without decoupled weight decay. |
| `RMSprop`, `Adagrad`, `Adadelta`, `Adamax` | The other adaptive methods. |
| `StepLR(stepSize, gamma)` | Multiply the rate by `gamma` every `stepSize` steps. |
| `MultiStepLR`, `ExponentialLR` | The same on several milestones, or every step. |
| `CosineAnnealingLR` | A cosine from the starting rate to zero. |
| `ReduceLROnPlateau` | Lower the rate when a metric stops improving. |

An optimizer is created from the model's parameters and then stepped after each
`backward`:

```kestrel
let mut optimizer := Adam(model.parameters(), 0.03)
optimizer.zeroGrad()
loss.backward()
optimizer.step()
```

## Data

| Call | Meaning |
|---|---|
| `TensorDataset(features, labels)` | Paired tensors. |
| `DataLoader(dataset, batchSize)` | Iterates it in batches. |
| `fromDataFrame(frame, features, target)` | Builds a dataset from a frame. |
| `trainTestSplit(dataset, testCount, seed = 0)` | Splits into train and test. |

## Notes

- Everything is eager: each operation runs when it is written, and the graph is
  recorded for `backward`.
- Gradients live on the `Variable`, not in a separate context.
- Training uses `float` tensors; a shape mismatch stops with a diagnostic that
  names both shapes.
