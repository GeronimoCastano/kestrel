# std.kesnet reference

Every public member of `std.kesnet`, `std.kesnet.data` and `std.kesnet.optim`.

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

- `Network`: The trait a module implements: a forward pass.
- `Module`: A neural-network block: a forward pass and registered parameters.
- `Identity`: A layer that returns its input.
- `Linear`: A dense layer.
- `ReLU`: The rectified linear unit.
- `LeakyReLU`: ReLU with a small slope below zero.
- `ELU`: The exponential linear unit.
- `GELU`: The Gaussian error linear unit.
- `SiLU`: The sigmoid linear unit.
- `Sigmoid`: The logistic sigmoid.
- `Tanh`: The hyperbolic tangent.
- `Softmax`: Softmax over one axis.
- `LogSoftmax`: The logarithm of softmax, computed stably.
- `Flatten`: Reshapes a tensor into one dimension per sample.
- `Dropout`: Randomly zeroes inputs during training.
- `Sequential`: A module that runs its children in order.
- `ModuleList`: A module that holds a list of children.
- `MSELoss`: Mean squared error as a loss module.
- `L1Loss`: Mean absolute error as a loss module.
- `CrossEntropyLoss`: Softmax plus negative log-likelihood.
- `NLLLoss`: Negative log-likelihood of normalised logits.
- `BCEWithLogitsLoss`: Binary cross-entropy from logits.
- `Conv2d`: A two-dimensional convolution layer.
- `MaxPool2d`: A 2-D max-pooling layer.
- `AvgPool2d`: A 2-D average-pooling layer.
- `AdaptiveAvgPool2d`: Average pooling to a fixed output size.
- `BatchNorm2d`: Batch normalisation over channels of a 4-D tensor.
- `BatchNorm1d`: Batch normalisation over the features of a 2-D tensor.
- `LayerNorm`: Normalisation over the last dimensions.
- `GroupNorm`: Normalisation over groups of channels.
- `Embedding`: A lookup table from indices to vectors.
- `MultiheadAttention`: Multi-head scaled dot-product attention.
- `TransformerEncoderLayer`: One transformer encoder block.
- `TransformerEncoder`: A stack of transformer encoder blocks.
- `LSTMCell`: One step of an LSTM.
- `LSTM`: An LSTM over a sequence.
- `GRUCell`: One step of a GRU.
- `GRU`: A GRU over a sequence.
- `RNNCell`: One step of a vanilla recurrent network.
- `RNN`: A recurrent network over a sequence.
- `MLP`: A multi-layer perceptron.

### Module

| Member | Signature | What it does |
|---|---|---|
| `addParameter` | `Module.addParameter(parameter : Variable) : void` | Registers a parameter with a module. |
| `parameters` | `Module.parameters() : List[Variable]` | The parameter variables of the module. |
| `parameterCount` | `Module.parameterCount() : Int` | How many parameters the module has. |
| `train` | `Module.train() : void` | Switches the module to training mode. |
| `eval` | `Module.eval() : void` | Switches the module to inference mode. |
| `zeroGrad` | `Module.zeroGrad() : void` | Clears the gradients. |
| `forward` | `Module.forward(x : Variable) : Variable` | The forward pass of the module. |
| `summary` | `Module.summary() : String` | A one-line summary of the module. |

### Identity

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Identity.forward(x : Variable) : Variable` | The forward pass of the module. |

### Linear

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Linear.forward(x : Variable) : Variable` | The forward pass of the module. |

### ReLU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `ReLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### LeakyReLU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `LeakyReLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### ELU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `ELU.forward(x : Variable) : Variable` | The forward pass of the module. |

### GELU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `GELU.forward(x : Variable) : Variable` | The forward pass of the module. |

### SiLU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `SiLU.forward(x : Variable) : Variable` | The forward pass of the module. |

### Sigmoid

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Sigmoid.forward(x : Variable) : Variable` | The forward pass of the module. |

### Tanh

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Tanh.forward(x : Variable) : Variable` | The forward pass of the module. |

### Softmax

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Softmax.forward(x : Variable) : Variable` | The forward pass of the module. |

### LogSoftmax

| Member | Signature | What it does |
|---|---|---|
| `forward` | `LogSoftmax.forward(x : Variable) : Variable` | The forward pass of the module. |

### Flatten

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Flatten.forward(x : Variable) : Variable` | The forward pass of the module. |

### Dropout

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Dropout.forward(x : Variable) : Variable` | The forward pass of the module. |

### Sequential

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Sequential.forward(x : Variable) : Variable` | The forward pass of the module. |

### ModuleList

| Member | Signature | What it does |
|---|---|---|
| `forward` | `ModuleList.forward(x : Variable) : Variable` | The forward pass of the module. |

### MSELoss

| Member | Signature | What it does |
|---|---|---|
| `forward` | `MSELoss.forward(prediction : Variable, target : Variable) : Variable` | The forward pass of the module. |

### L1Loss

| Member | Signature | What it does |
|---|---|---|
| `forward` | `L1Loss.forward(prediction : Variable, target : Variable) : Variable` | The forward pass of the module. |

### CrossEntropyLoss

| Member | Signature | What it does |
|---|---|---|
| `forward` | `CrossEntropyLoss.forward(logits : Variable, target : Variable) : Variable` | The forward pass of the module. |

### NLLLoss

| Member | Signature | What it does |
|---|---|---|
| `forward` | `NLLLoss.forward(logProbabilities : Variable, target : Variable) : Variable` | The forward pass of the module. |

### BCEWithLogitsLoss

| Member | Signature | What it does |
|---|---|---|
| `forward` | `BCEWithLogitsLoss.forward(logits : Variable, target : Variable) : Variable` | The forward pass of the module. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `relu` | `std.kesnet.relu(x : Variable) : Variable` | The ReLU activation. |
| `sigmoid` | `std.kesnet.sigmoid(x : Variable) : Variable` | The sigmoid activation. |
| `tanh` | `std.kesnet.tanh(x : Variable) : Variable` | The hyperbolic tangent. |
| `gelu` | `std.kesnet.gelu(x : Variable) : Variable` | The GELU activation. |
| `silu` | `std.kesnet.silu(x : Variable) : Variable` | The SiLU activation. |
| `softmax` | `std.kesnet.softmax(x : Variable, axis : Int) : Variable` | The softmax activation. |
| `logSoftmax` | `std.kesnet.logSoftmax(x : Variable, axis : Int) : Variable` | The logarithm of softmax, computed stably. |
| `linear` | `std.kesnet.linear(x : Variable, weight : Variable, bias : Option[Variable]) : Variable` | A linear transform of a variable. |
| `mseLoss` | `std.kesnet.mseLoss(prediction : Variable, target : Variable) : Variable` | Mean squared error as a loss value. |
| `l1Loss` | `std.kesnet.l1Loss(prediction : Variable, target : Variable) : Variable` | Mean absolute error as a loss value. |
| `crossEntropy` | `std.kesnet.crossEntropy(logits : Variable, target : Variable) : Variable` | Softmax then negative log-likelihood. |
| `nllLoss` | `std.kesnet.nllLoss(logProbabilities : Variable, target : Variable) : Variable` | Negative log-likelihood of normalised logits. |
| `binaryCrossEntropy` | `std.kesnet.binaryCrossEntropy(logits : Variable, target : Variable) : Variable` | Binary cross-entropy from logits. |
| `cat` | `std.kesnet.cat(values : List[Variable], axis : Int) : Variable` | Joins variables along an axis. |
| `stack` | `std.kesnet.stack(values : List[Variable], axis : Int) : Variable` | Stacks values along a new axis. |
| `saveParameters` | `std.kesnet.saveParameters(module : Module, path : String) : void with Effects` | Saves the module parameters to a path. |
| `loadParameters` | `std.kesnet.loadParameters(module : Module, path : String) : void with Effects` | Loads saved parameters into a module. |
| `accuracy` | `std.kesnet.accuracy(logits : Variable, targets : Variable) : double` | The fraction of predictions that equal the labels. |
| `meanAbsoluteError` | `std.kesnet.meanAbsoluteError(prediction : Variable, target : Variable) : double` | The mean absolute error. |
| `meanSquaredError` | `std.kesnet.meanSquaredError(prediction : Variable, target : Variable) : double` | The mean squared error. |
| `xavierUniform` | `std.kesnet.xavierUniform(weight : Variable) : void` | Xavier uniform initialisation. |
| `kaimingUniform` | `std.kesnet.kaimingUniform(weight : Variable) : void` | Kaiming uniform initialisation. |
| `normalInit` | `std.kesnet.normalInit(weight : Variable, mean : double, sigma : double) : void` | Normal initialisation. |
| `zerosInit` | `std.kesnet.zerosInit(weight : Variable) : void` | Initialisation with zeros. |
| `onesInit` | `std.kesnet.onesInit(weight : Variable) : void` | Initialisation with ones. |

### Conv2d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Conv2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### MaxPool2d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `MaxPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### AvgPool2d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `AvgPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### AdaptiveAvgPool2d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `AdaptiveAvgPool2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### BatchNorm2d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `BatchNorm2d.forward(x : Variable) : Variable` | The forward pass of the module. |

### BatchNorm1d

| Member | Signature | What it does |
|---|---|---|
| `forward` | `BatchNorm1d.forward(x : Variable) : Variable` | The forward pass of the module. |

### LayerNorm

| Member | Signature | What it does |
|---|---|---|
| `forward` | `LayerNorm.forward(x : Variable) : Variable` | The forward pass of the module. |

### GroupNorm

| Member | Signature | What it does |
|---|---|---|
| `forward` | `GroupNorm.forward(x : Variable) : Variable` | The forward pass of the module. |

### Embedding

| Member | Signature | What it does |
|---|---|---|
| `forward` | `Embedding.forward(x : Variable) : Variable` | The forward pass of the module. |

### MultiheadAttention

| Member | Signature | What it does |
|---|---|---|
| `forward` | `MultiheadAttention.forward(x : Variable) : Variable` | The forward pass of the module. |

### TransformerEncoderLayer

| Member | Signature | What it does |
|---|---|---|
| `forward` | `TransformerEncoderLayer.forward(x : Variable) : Variable` | The forward pass of the module. |

### TransformerEncoder

| Member | Signature | What it does |
|---|---|---|
| `forward` | `TransformerEncoder.forward(x : Variable) : Variable` | The forward pass of the module. |

### LSTMCell

| Member | Signature | What it does |
|---|---|---|
| `step` | `LSTMCell.step(x : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)` | A step plot. |
| `stepProjected` | `LSTMCell.stepProjected(projected : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)` | A step plot drawn in projection. |
| `zeros` | `LSTMCell.zeros(batch : Int) : (Variable, Variable)` | A tensor of zeros. |

### LSTM

| Member | Signature | What it does |
|---|---|---|
| `forward` | `LSTM.forward(x : Variable) : Variable` | The forward pass of the module. |

### GRUCell

| Member | Signature | What it does |
|---|---|---|
| `step` | `GRUCell.step(x : Variable, hidden : Variable) : Variable` | A step plot. |
| `zeros` | `GRUCell.zeros(batch : Int) : Variable` | A tensor of zeros. |

### GRU

| Member | Signature | What it does |
|---|---|---|
| `forward` | `GRU.forward(x : Variable) : Variable` | The forward pass of the module. |

### RNNCell

| Member | Signature | What it does |
|---|---|---|
| `step` | `RNNCell.step(x : Variable, hidden : Variable) : Variable` | A step plot. |
| `zeros` | `RNNCell.zeros(batch : Int) : Variable` | A tensor of zeros. |

### RNN

| Member | Signature | What it does |
|---|---|---|
| `forward` | `RNN.forward(x : Variable) : Variable` | The forward pass of the module. |

### MLP

| Member | Signature | What it does |
|---|---|---|
| `forward` | `MLP.forward(x : Variable) : Variable` | The forward pass of the module. |

## kesnet optim

### Types

- `Optimizer`: The base class of the gradient-descent optimizers.
- `SGD`: Gradient descent, with optional momentum.
- `Adam`: The Adam optimizer.
- `AdamW`: Adam with decoupled weight decay.
- `RMSprop`: The RMSprop optimizer.
- `Adagrad`: The Adagrad optimizer.
- `Adadelta`: The Adadelta optimizer.
- `Adamax`: The Adamax optimizer.
- `StepLR`: A schedule that scales the rate every fixed number of steps.
- `MultiStepLR`: A schedule that scales the rate at several milestones.
- `ExponentialLR`: A schedule that scales the rate every step.
- `CosineAnnealingLR`: A cosine schedule from the starting rate to zero.
- `ReduceLROnPlateau`: A schedule that lowers the rate when a metric stalls.

### Optimizer

| Member | Signature | What it does |
|---|---|---|
| `zeroGrad` | `Optimizer.zeroGrad() : void` | Clears the gradients. |
| `setLearningRate` | `Optimizer.setLearningRate(value : double) : void` | Sets the learning rate. |
| `step` | `Optimizer.step() : void` | A step plot. |
| `parameterCount` | `Optimizer.parameterCount() : Int` | How many parameters the module has. |

### SGD

| Member | Signature | What it does |
|---|---|---|
| `step` | `SGD.step() : void` | A step plot. |

### Adam

| Member | Signature | What it does |
|---|---|---|
| `step` | `Adam.step() : void` | A step plot. |

### AdamW

| Member | Signature | What it does |
|---|---|---|
| `step` | `AdamW.step() : void` | A step plot. |

### RMSprop

| Member | Signature | What it does |
|---|---|---|
| `step` | `RMSprop.step() : void` | A step plot. |

### Adagrad

| Member | Signature | What it does |
|---|---|---|
| `step` | `Adagrad.step() : void` | A step plot. |

### Adadelta

| Member | Signature | What it does |
|---|---|---|
| `step` | `Adadelta.step() : void` | A step plot. |

### Adamax

| Member | Signature | What it does |
|---|---|---|
| `step` | `Adamax.step() : void` | A step plot. |

### StepLR

| Member | Signature | What it does |
|---|---|---|
| `step` | `StepLR.step(optimizer : Optimizer) : void` | A step plot. |
| `lastLearningRate` | `StepLR.lastLearningRate(optimizer : Optimizer) : double` | The learning rate the optimizer used last. |

### MultiStepLR

| Member | Signature | What it does |
|---|---|---|
| `step` | `MultiStepLR.step(optimizer : Optimizer) : void` | A step plot. |

### ExponentialLR

| Member | Signature | What it does |
|---|---|---|
| `step` | `ExponentialLR.step(optimizer : Optimizer) : void` | A step plot. |

### CosineAnnealingLR

| Member | Signature | What it does |
|---|---|---|
| `step` | `CosineAnnealingLR.step(optimizer : Optimizer) : void` | A step plot. |

### ReduceLROnPlateau

| Member | Signature | What it does |
|---|---|---|
| `step` | `ReduceLROnPlateau.step(optimizer : Optimizer, metric : double) : void` | A step plot. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `warmupRate` | `std.kesnet.optim.warmupRate(base : double, step : Int, warmupSteps : Int) : double` | A learning rate that warms up over the first steps. |

## kesnet data

### Types

- `TensorDataset`: Paired feature and label tensors.
- `DataLoader`: Iterates a dataset in batches.

### TensorDataset

| Member | Signature | What it does |
|---|---|---|
| `length` | `TensorDataset.length() : Int` | How many values. |
| `featureCount` | `TensorDataset.featureCount() : Int` | How many features the dataset has. |
| `targetCount` | `TensorDataset.targetCount() : Int` | The number of target values. |
| `targetRank` | `TensorDataset.targetRank() : Int` | The rank of the target matrix. |

### DataLoader

| Member | Signature | What it does |
|---|---|---|
| `batches` | `DataLoader.batches() : Array[(Variable, Variable)]` | The batches of a data loader. |
| `length` | `DataLoader.length() : Int` | How many values. |
| `reset` | `DataLoader.reset() : void` | Resets the state. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `fromDataFrame` | `std.kesnet.data.fromDataFrame(frame : DataFrame, features : List[String], target : String) : TensorDataset or Error` | A dataset built from a frame. |
| `trainTestSplit` | `std.kesnet.data.trainTestSplit(dataset : TensorDataset, testCount : Int, seed : Int) : (TensorDataset, TensorDataset)` | Splits a dataset into training and test parts. |
