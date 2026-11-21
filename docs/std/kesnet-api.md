# std.kesnet reference

Part of [`std.kesnet`](kesnet.md); this page lists its members, the guide explains the workflow.

Every public member of `std.kesnet`, `std.kesnet.data` and `std.kesnet.optim`.

Three module paths make up the library, and each binds the last part of its
path when it is imported:

| Path | Import | Names in code |
|---|---|---|
| `std.kesnet` | `import std.kesnet` | `kesnet.Linear`, `kesnet.mseLoss` |
| `std.kesnet.data` | `import std.kesnet.data` | `data.TensorDataset`, `data.fromDataFrame` |
| `std.kesnet.optim` | `import std.kesnet.optim` | `optim.Adam`, `optim.StepLR` |

A selective import brings the names in unqualified: after
`import std.kesnet.optim.{Adam, StepLR}` you write `Adam(...)` and `StepLR(...)`.
The `std.` prefix is optional, so `import kesnet.optim.{Adam}` is the same import.
In the tables below, `kesnet.optim.Adam` is the full path from the package root.

A model chains too: `Sequential([...])` of layers, then `forward`, then a loss,
then `backward` and `step` on the optimizer.

```kestrel
let model := Sequential([box(Linear(2, 16)), box(ReLU()), box(Linear(16, 1))])
let loss := mseLoss(model.forward(inputs), expected)
optimizer.zeroGrad()
loss.backward()
optimizer.step()
```


## kesnet

### Types

- `kesnet.Network`: The trait a module implements: a forward pass.
- `kesnet.Module`: A neural-network block: a forward pass and registered parameters.
- `kesnet.Identity`: A layer that returns its input.
- `kesnet.Linear`: A dense layer.
- `kesnet.ReLU`: The rectified linear unit.
- `kesnet.LeakyReLU`: ReLU with a small slope below zero.
- `kesnet.ELU`: The exponential linear unit.
- `kesnet.GELU`: The Gaussian error linear unit.
- `kesnet.SiLU`: The sigmoid linear unit.
- `kesnet.Sigmoid`: The logistic sigmoid.
- `kesnet.Tanh`: The hyperbolic tangent.
- `kesnet.Softmax`: Softmax over one axis.
- `kesnet.LogSoftmax`: The logarithm of softmax, computed stably.
- `kesnet.Flatten`: Reshapes a tensor into one dimension per sample.
- `kesnet.Dropout`: Randomly zeroes inputs during training.
- `kesnet.Sequential`: A module that runs its children in order.
- `kesnet.ModuleList`: A module that holds a list of children.
- `kesnet.MSELoss`: Mean squared error as a loss module.
- `kesnet.L1Loss`: Mean absolute error as a loss module.
- `kesnet.CrossEntropyLoss`: Softmax plus negative log-likelihood.
- `kesnet.NLLLoss`: Negative log-likelihood of normalised logits.
- `kesnet.BCEWithLogitsLoss`: Binary cross-entropy from logits.
- `kesnet.Conv2d`: A two-dimensional convolution layer.
- `kesnet.MaxPool2d`: A 2-D max-pooling layer.
- `kesnet.AvgPool2d`: A 2-D average-pooling layer.
- `kesnet.AdaptiveAvgPool2d`: Average pooling to a fixed output size.
- `kesnet.BatchNorm2d`: Batch normalisation over channels of a 4-D tensor.
- `kesnet.BatchNorm1d`: Batch normalisation over the features of a 2-D tensor.
- `kesnet.LayerNorm`: Normalisation over the last dimensions.
- `kesnet.GroupNorm`: Normalisation over groups of channels.
- `kesnet.Embedding`: A lookup table from indices to vectors.
- `kesnet.MultiheadAttention`: Multi-head scaled dot-product attention.
- `kesnet.TransformerEncoderLayer`: One transformer encoder block.
- `kesnet.TransformerEncoder`: A stack of transformer encoder blocks.
- `kesnet.LSTMCell`: One step of an LSTM.
- `kesnet.LSTM`: An LSTM over a sequence.
- `kesnet.GRUCell`: One step of a GRU.
- `kesnet.GRU`: A GRU over a sequence.
- `kesnet.RNNCell`: One step of a vanilla recurrent network.
- `kesnet.RNN`: A recurrent network over a sequence.
- `kesnet.MLP`: A multi-layer perceptron.

### Module

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Module.addParameter` | `Module.addParameter(parameter : Variable) : void` | Registers a parameter with a module. |
| `kesnet.Module.parameters` | `Module.parameters() : List[Variable]` | The parameter variables of the module. |
| `kesnet.Module.parameterCount` | `Module.parameterCount() : Int` | How many parameters the module has. |
| `kesnet.Module.train` | `Module.train() : void` | Switches the module to training mode. |
| `kesnet.Module.eval` | `Module.eval() : void` | Switches the module to inference mode. |
| `kesnet.Module.zeroGrad` | `Module.zeroGrad() : void` | Clears the gradients. |
| `kesnet.Module.forward` | `Module.forward(x : Variable) : Variable` | The forward pass of the module. |
| `kesnet.Module.summary` | `Module.summary() : String` | A one-line summary of the module. |

### Identity

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Identity.forward` | `Identity.forward(x : Variable) : Variable` | The forward pass of the module. |

### Linear

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Linear.forward` | `Linear.forward(x : Variable) : Variable` | The forward pass of the module. |

### ReLU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.ReLU.forward` | `ReLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### LeakyReLU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.LeakyReLU.forward` | `LeakyReLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### ELU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.ELU.forward` | `ELU.forward(x : Variable) : Variable` | The forward pass of the module. |

### GELU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.GELU.forward` | `GELU.forward(x : Variable) : Variable` | The forward pass of the module. |

### SiLU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.SiLU.forward` | `SiLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### Sigmoid

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Sigmoid.forward` | `Sigmoid.forward(x : Variable) : Variable` | The forward pass of the module. |

### Tanh

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Tanh.forward` | `Tanh.forward(x : Variable) : Variable` | The forward pass of the module. |

### Softmax

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Softmax.forward` | `Softmax.forward(x : Variable) : Variable` | The forward pass of the module. |

### LogSoftmax

| Member | Signature | What it does |
|---|---|---|
| `kesnet.LogSoftmax.forward` | `LogSoftmax.forward(x : Variable) : Variable` | The forward pass of the module. |

### Flatten

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Flatten.forward` | `Flatten.forward(x : Variable) : Variable` | The forward pass of the module. |

### Dropout

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Dropout.forward` | `Dropout.forward(x : Variable) : Variable` | The forward pass of the module. |

### Sequential

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Sequential.forward` | `Sequential.forward(x : Variable) : Variable` | The forward pass of the module. |

### ModuleList

| Member | Signature | What it does |
|---|---|---|
| `kesnet.ModuleList.forward` | `ModuleList.forward(x : Variable) : Variable` | The forward pass of the module. |

### MSELoss

| Member | Signature | What it does |
|---|---|---|
| `kesnet.MSELoss.forward` | `MSELoss.forward(prediction : Variable, target : Variable) : Variable` | The forward pass of the module. |

### L1Loss

| Member | Signature | What it does |
|---|---|---|
| `kesnet.L1Loss.forward` | `L1Loss.forward(prediction : Variable, target : Variable) : Variable` | The forward pass of the module. |

### CrossEntropyLoss

| Member | Signature | What it does |
|---|---|---|
| `kesnet.CrossEntropyLoss.forward` | `CrossEntropyLoss.forward(logits : Variable, target : Variable) : Variable` | The forward pass of the module. |

### NLLLoss

| Member | Signature | What it does |
|---|---|---|
| `kesnet.NLLLoss.forward` | `NLLLoss.forward(logProbabilities : Variable, target : Variable) : Variable` | The forward pass of the module. |

### BCEWithLogitsLoss

| Member | Signature | What it does |
|---|---|---|
| `kesnet.BCEWithLogitsLoss.forward` | `BCEWithLogitsLoss.forward(logits : Variable, target : Variable) : Variable` | The forward pass of the module. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `kesnet.relu` | `std.kesnet.relu(x : Variable) : Variable` | The ReLU activation. |
| `kesnet.sigmoid` | `std.kesnet.sigmoid(x : Variable) : Variable` | The sigmoid activation. |
| `kesnet.tanh` | `std.kesnet.tanh(x : Variable) : Variable` | The hyperbolic tangent. |
| `kesnet.gelu` | `std.kesnet.gelu(x : Variable) : Variable` | The GELU activation. |
| `kesnet.silu` | `std.kesnet.silu(x : Variable) : Variable` | The SiLU activation. |
| `kesnet.softmax` | `std.kesnet.softmax(x : Variable, axis : Int) : Variable` | The softmax activation. |
| `kesnet.logSoftmax` | `std.kesnet.logSoftmax(x : Variable, axis : Int) : Variable` | The logarithm of softmax, computed stably. |
| `kesnet.linear` | `std.kesnet.linear(x : Variable, weight : Variable, bias : Option[Variable]) : Variable` | A linear transform of a variable. |
| `kesnet.mseLoss` | `std.kesnet.mseLoss(prediction : Variable, target : Variable) : Variable` | Mean squared error as a loss value. |
| `kesnet.l1Loss` | `std.kesnet.l1Loss(prediction : Variable, target : Variable) : Variable` | Mean absolute error as a loss value. |
| `kesnet.crossEntropy` | `std.kesnet.crossEntropy(logits : Variable, target : Variable) : Variable` | Softmax then negative log-likelihood. |
| `kesnet.nllLoss` | `std.kesnet.nllLoss(logProbabilities : Variable, target : Variable) : Variable` | Negative log-likelihood of normalised logits. |
| `kesnet.binaryCrossEntropy` | `std.kesnet.binaryCrossEntropy(logits : Variable, target : Variable) : Variable` | Binary cross-entropy from logits. |
| `kesnet.cat` | `std.kesnet.cat(values : List[Variable], axis : Int) : Variable` | Joins variables along an axis. |
| `kesnet.stack` | `std.kesnet.stack(values : List[Variable], axis : Int) : Variable` | Stacks values along a new axis. |
| `kesnet.saveParameters` | `std.kesnet.saveParameters(module : Module, path : String) : void with Effects` | Saves the module parameters to a path. |
| `kesnet.loadParameters` | `std.kesnet.loadParameters(module : Module, path : String) : void with Effects` | Loads saved parameters into a module. |
| `kesnet.accuracy` | `std.kesnet.accuracy(logits : Variable, targets : Variable) : double` | The fraction of predictions that equal the labels. |
| `kesnet.meanAbsoluteError` | `std.kesnet.meanAbsoluteError(prediction : Variable, target : Variable) : double` | The mean absolute error. |
| `kesnet.meanSquaredError` | `std.kesnet.meanSquaredError(prediction : Variable, target : Variable) : double` | The mean squared error. |
| `kesnet.xavierUniform` | `std.kesnet.xavierUniform(weight : Variable) : void` | Xavier uniform initialisation. |
| `kesnet.kaimingUniform` | `std.kesnet.kaimingUniform(weight : Variable) : void` | Kaiming uniform initialisation. |
| `kesnet.normalInit` | `std.kesnet.normalInit(weight : Variable, mean : double, sigma : double) : void` | Normal initialisation. |
| `kesnet.zerosInit` | `std.kesnet.zerosInit(weight : Variable) : void` | Initialisation with zeros. |
| `kesnet.onesInit` | `std.kesnet.onesInit(weight : Variable) : void` | Initialisation with ones. |

### Conv2d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Conv2d.forward` | `Conv2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### MaxPool2d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.MaxPool2d.forward` | `MaxPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### AvgPool2d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.AvgPool2d.forward` | `AvgPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### AdaptiveAvgPool2d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.AdaptiveAvgPool2d.forward` | `AdaptiveAvgPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### BatchNorm2d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.BatchNorm2d.forward` | `BatchNorm2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### BatchNorm1d

| Member | Signature | What it does |
|---|---|---|
| `kesnet.BatchNorm1d.forward` | `BatchNorm1d.forward(x : Variable) : Variable` | The forward pass of the module. |

### LayerNorm

| Member | Signature | What it does |
|---|---|---|
| `kesnet.LayerNorm.forward` | `LayerNorm.forward(x : Variable) : Variable` | The forward pass of the module. |

### GroupNorm

| Member | Signature | What it does |
|---|---|---|
| `kesnet.GroupNorm.forward` | `GroupNorm.forward(x : Variable) : Variable` | The forward pass of the module. |

### Embedding

| Member | Signature | What it does |
|---|---|---|
| `kesnet.Embedding.forward` | `Embedding.forward(x : Variable) : Variable` | The forward pass of the module. |

### MultiheadAttention

| Member | Signature | What it does |
|---|---|---|
| `kesnet.MultiheadAttention.forward` | `MultiheadAttention.forward(x : Variable) : Variable` | The forward pass of the module. |

### TransformerEncoderLayer

| Member | Signature | What it does |
|---|---|---|
| `kesnet.TransformerEncoderLayer.forward` | `TransformerEncoderLayer.forward(x : Variable) : Variable` | The forward pass of the module. |

### TransformerEncoder

| Member | Signature | What it does |
|---|---|---|
| `kesnet.TransformerEncoder.forward` | `TransformerEncoder.forward(x : Variable) : Variable` | The forward pass of the module. |

### LSTMCell

| Member | Signature | What it does |
|---|---|---|
| `kesnet.LSTMCell.step` | `LSTMCell.step(x : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)` | A step plot. |
| `kesnet.LSTMCell.stepProjected` | `LSTMCell.stepProjected(projected : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)` | A step plot drawn in projection. |
| `kesnet.LSTMCell.zeros` | `LSTMCell.zeros(batch : Int) : (Variable, Variable)` | A tensor of zeros. |

### LSTM

| Member | Signature | What it does |
|---|---|---|
| `kesnet.LSTM.forward` | `LSTM.forward(x : Variable) : Variable` | The forward pass of the module. |

### GRUCell

| Member | Signature | What it does |
|---|---|---|
| `kesnet.GRUCell.step` | `GRUCell.step(x : Variable, hidden : Variable) : Variable` | A step plot. |
| `kesnet.GRUCell.zeros` | `GRUCell.zeros(batch : Int) : Variable` | A tensor of zeros. |

### GRU

| Member | Signature | What it does |
|---|---|---|
| `kesnet.GRU.forward` | `GRU.forward(x : Variable) : Variable` | The forward pass of the module. |

### RNNCell

| Member | Signature | What it does |
|---|---|---|
| `kesnet.RNNCell.step` | `RNNCell.step(x : Variable, hidden : Variable) : Variable` | A step plot. |
| `kesnet.RNNCell.zeros` | `RNNCell.zeros(batch : Int) : Variable` | A tensor of zeros. |

### RNN

| Member | Signature | What it does |
|---|---|---|
| `kesnet.RNN.forward` | `RNN.forward(x : Variable) : Variable` | The forward pass of the module. |

### MLP

| Member | Signature | What it does |
|---|---|---|
| `kesnet.MLP.forward` | `MLP.forward(x : Variable) : Variable` | The forward pass of the module. |

## kesnet.optim

### Types

- `kesnet.optim.Optimizer`: The base class of the gradient-descent optimizers.
- `kesnet.optim.SGD`: Gradient descent, with optional momentum.
- `kesnet.optim.Adam`: The Adam optimizer.
- `kesnet.optim.AdamW`: Adam with decoupled weight decay.
- `kesnet.optim.RMSprop`: The RMSprop optimizer.
- `kesnet.optim.Adagrad`: The Adagrad optimizer.
- `kesnet.optim.Adadelta`: The Adadelta optimizer.
- `kesnet.optim.Adamax`: The Adamax optimizer.
- `kesnet.optim.StepLR`: A schedule that scales the rate every fixed number of steps.
- `kesnet.optim.MultiStepLR`: A schedule that scales the rate at several milestones.
- `kesnet.optim.ExponentialLR`: A schedule that scales the rate every step.
- `kesnet.optim.CosineAnnealingLR`: A cosine schedule from the starting rate to zero.
- `kesnet.optim.ReduceLROnPlateau`: A schedule that lowers the rate when a metric stalls.

### Optimizer

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.Optimizer.zeroGrad` | `Optimizer.zeroGrad() : void` | Clears the gradients. |
| `kesnet.optim.Optimizer.setLearningRate` | `Optimizer.setLearningRate(value : double) : void` | Sets the learning rate. |
| `kesnet.optim.Optimizer.step` | `Optimizer.step() : void` | A step plot. |
| `kesnet.optim.Optimizer.parameterCount` | `Optimizer.parameterCount() : Int` | How many parameters the module has. |

### SGD

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.SGD.step` | `SGD.step() : void` | A step plot. |

### Adam

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.Adam.step` | `Adam.step() : void` | A step plot. |

### AdamW

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.AdamW.step` | `AdamW.step() : void` | A step plot. |

### RMSprop

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.RMSprop.step` | `RMSprop.step() : void` | A step plot. |

### Adagrad

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.Adagrad.step` | `Adagrad.step() : void` | A step plot. |

### Adadelta

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.Adadelta.step` | `Adadelta.step() : void` | A step plot. |

### Adamax

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.Adamax.step` | `Adamax.step() : void` | A step plot. |

### StepLR

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.StepLR.step` | `StepLR.step(optimizer : Optimizer) : void` | A step plot. |
| `kesnet.optim.StepLR.lastLearningRate` | `StepLR.lastLearningRate(optimizer : Optimizer) : double` | The learning rate the optimizer used last. |

### MultiStepLR

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.MultiStepLR.step` | `MultiStepLR.step(optimizer : Optimizer) : void` | A step plot. |

### ExponentialLR

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.ExponentialLR.step` | `ExponentialLR.step(optimizer : Optimizer) : void` | A step plot. |

### CosineAnnealingLR

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.CosineAnnealingLR.step` | `CosineAnnealingLR.step(optimizer : Optimizer) : void` | A step plot. |

### ReduceLROnPlateau

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.ReduceLROnPlateau.step` | `ReduceLROnPlateau.step(optimizer : Optimizer, metric : double) : void` | A step plot. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `kesnet.optim.warmupRate` | `std.kesnet.optim.warmupRate(base : double, step : Int, warmupSteps : Int) : double` | A learning rate that warms up over the first steps. |

## kesnet.data

### Types

- `kesnet.data.TensorDataset`: Paired feature and label tensors.
- `kesnet.data.DataLoader`: Iterates a dataset in batches.

### TensorDataset

| Member | Signature | What it does |
|---|---|---|
| `kesnet.data.TensorDataset.length` | `TensorDataset.length() : Int` | How many values. |
| `kesnet.data.TensorDataset.featureCount` | `TensorDataset.featureCount() : Int` | How many features the dataset has. |
| `kesnet.data.TensorDataset.targetCount` | `TensorDataset.targetCount() : Int` | The number of target values. |
| `kesnet.data.TensorDataset.targetRank` | `TensorDataset.targetRank() : Int` | The rank of the target matrix. |

### DataLoader

| Member | Signature | What it does |
|---|---|---|
| `kesnet.data.DataLoader.batches` | `DataLoader.batches() : Array[(Variable, Variable)]` | The batches of a data loader. |
| `kesnet.data.DataLoader.length` | `DataLoader.length() : Int` | How many values. |
| `kesnet.data.DataLoader.reset` | `DataLoader.reset() : void` | Resets the state. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `kesnet.data.fromDataFrame` | `std.kesnet.data.fromDataFrame(frame : DataFrame, features : List[String], target : String) : TensorDataset or Error` | A dataset built from a frame. |
| `kesnet.data.trainTestSplit` | `std.kesnet.data.trainTestSplit(dataset : TensorDataset, testCount : Int, seed : Int) : (TensorDataset, TensorDataset)` | Splits a dataset into training and test parts. |
