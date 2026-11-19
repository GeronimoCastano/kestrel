# KesNet

KesNet is Kestrel's deep-learning library: tensors that record operations, a
reverse-mode autograd engine, neural-network layers, losses, optimizers, data
loaders, and the editor support that makes the types visible while you write.
The name joins **Kes**trel with a neural **net**. The module is `std.kesnet`.

It is deliberately PyTorch-shaped. A model is a class extending `Module`, its
layers and parameters are fields, and `forward` describes the computation. The
set of operations is smaller than PyTorch's, but the parts needed for a real
training loop are native code, not a wrapper around another framework.

- [Quick start](#quick-start)
- [Defining a model](#defining-a-model)
- [Variables and autograd](#variables-and-autograd)
- [Layers](#layers)
- [Losses](#losses)
- [Optimizers and schedules](#optimizers-and-schedules)
- [Data](#data)
- [Serialization](#serialization)
- [Editor support](#editor-support)
- [Design](#design)
- [Limits](#limits)
- [Examples](#examples)

## Quick start

```kestrel
import std.kesnet.{Linear, ReLU, Sequential, mseLoss}
import std.kesnet.optim.{Adam}

let x := Variable.fromArray([1.0, 2.0, 2.0, 1.0, 4.0, 3.0, 5.0, 6.0].toArray(), (4, 2))
let y := Variable.fromArray([8.0, 7.0, 18.0, 29.0].toArray(), (4, 1))

let model := Sequential([box(Linear(2, 16)), box(ReLU()), box(Linear(16, 1))])
let mut optimizer := Adam(model.parameters(), 0.03)

for (epoch in range(400)) {
    let prediction := model.forward(x)
    let loss := mseLoss(prediction, y)
    optimizer.zeroGrad()
    loss.backward()
    optimizer.step()
}

print(mseLoss(model.forward(x), y).item())
```

`Variable` is the differentiable tensor. `Tensor` and `DynTensor` remain the
plain data tensors; `Variable.fromTensor` and `Variable.fromArray` build a
differentiable value from them.

## Defining a model

```kestrel
import std.kesnet.{Module, Linear, ReLU, Variable}

public class TwoLayerNet extends Module {
    public var first : Linear
    public var second : Linear
    public var hidden : ReLU

    new(inputs : Int, hiddenSize : Int, outputs : Int) {
        this.registered = Array.empty()
        this.trainingMode = true
        this.first = Linear(inputs, hiddenSize)
        this.second = Linear(hiddenSize, outputs)
        this.hidden = ReLU()
        let firstWeight := this.first.weight
        let firstBias := this.first.biasValue
        let secondWeight := this.second.weight
        let secondBias := this.second.biasValue
        this.registered = [firstWeight, firstBias, secondWeight, secondBias].toArray()
    }

    @override public function forward(x : Variable) : Variable {
        this.second.forward(this.hidden.forward(this.first.forward(x)))
    }
}

let model := TwoLayerNet(2, 32, 1)
let parameters := model.parameters()
```

Kestrel has no reflection over fields, so parameters are registered explicitly.
`Module` holds an `Array[Variable]`; `parameters()` returns a `List` of copies.
A copy of a `Variable` shares its native node, which is what lets an optimizer
update the same parameters the model uses. `train()` and `eval()` also set the
global training flag read by `Dropout` and `BatchNorm`.

`Sequential` accepts a `List[Box[dyn Network]]`; `box(...)` puts a layer behind
the `Network` trait so layers of different classes can sit in one list.

## Variables and autograd

- `Variable.zeros`, `ones`, `filled`, `uniform`, `normal`, `bernoulli`, `randn`,
  `rand`, `randint`, `eye`, `arange`, `linspace`, `fromArray`, `fromTensor`,
  `scalar`, `parameter`, `embedding`, `zerosLike`, `onesLike`, `randLike`, and
  `load` build values. The free functions `cat(values, axis)` and
  `stack(values, axis)` join a list of values.
- `x.backward()` seeds the graph with ones. Call it on a scalar loss after
  `optimizer.zeroGrad()` or `x.zeroGrad()`.
- Leaf gradients are stored on the leaf. `x.grad()` returns
  `Option[DynTensor[double]]`; `x.gradVariable()` returns a `Variable` of zeros
  when there is no gradient, which is convenient in optimizers.
- Non-leaf gradients are released after the reverse pass unless
  `x.retainGrad()` was called. The graph itself is reference counted: when the
  last handle to a result is dropped, its saved inputs and parent references
  are released. A training loop does not accumulate graph nodes.
- `Variable.setGradEnabled(false)` and `Variable.isGradEnabled()` switch the
  global recording mode. Operations still compute values, but they do not build
  a graph.
- `x.detach()` copies data into a new leaf. `x.clone()` is a differentiable
  identity copy.
- `Variable.liveNodes()` and `Variable.liveBytes()` report the native node
  count and accounted bytes. They exist for tests and profiles, not for model
  logic.

`Variable` is copyable: assignment and passing it to a function copy the handle.
`Variable.liveNodes()` makes the sharing visible. This differs from Kestrel
classes, which move.

Arithmetic operators follow PyTorch's meaning:

| Expression | Result |
|---|---|
| `x + y`, `x - y`, `x * y`, `x / y` | elementwise with NumPy broadcasting |
| `x + 2.0`, `2.0 * x` | scalar on either side |
| `x @ y` | matrix or matrix-vector product |
| `x.bmm(y)` | batched matrix product over leading dimensions |
| `x ** 2.0` | elementwise power |
| `-x` | elementwise negation |

Indexing (`x[0]`, `x[i, j]`) removes dimensions. `slice(axis, start, end)`
keeps a range. Shape operations are `reshape`, `flatten[startDim]`,
`transpose(i, j)`, `permute(axes)`, `squeeze`, `unsqueeze`, `broadcastTo`, `expand`, and `cat`.
`cat(values, axis)` and `stack(values, axis)` in `std.kesnet` fold a list.

Reductions are `sum`, `mean`, `prod`, `min`, `max`, `argmin`, and `argmax`, with
an optional axis and `keepdim`. `norm()` is the L2 norm over every element.

The dtype is `float64` by default. `x.to("float32")`, `x.to("float16")`, and
`x.to("bfloat16")` round values to that precision; `x.dtype()` reports the name.
Devices are modelled as `cpu`; `x.to("mps")` and `x.to("cuda")` stop with a
clear message in this build (see [Limits](#limits)).

## Layers

All layers extend `Module` and implement `forward(x : Variable) : Variable`
unless noted.

| Layer | Constructor | Notes |
|---|---|---|
| `Linear` | `Linear(in, out, bias = true)` | folds leading axes into rows, so it accepts `(rows, in)` and `(batch, time, in)` |
| `Conv2d` | `Conv2d(in, out, kernel, stride = 1, padding = 0, dilation = 1, groups = 1, bias = true)` | input `(N, C, H, W)`; square kernel, stride, padding and dilation |
| `MaxPool2d` | `MaxPool2d(kernel, stride = kernel, padding = 0)` | saves argmax positions for backward |
| `AvgPool2d` | `AvgPool2d(kernel, stride = kernel, padding = 0)` | divides by the valid cell count |
| `AdaptiveAvgPool2d` | `AdaptiveAvgPool2d(outHeight, outWidth)` | overlapping bins when the input is not divisible |
| `BatchNorm2d` | `BatchNorm2d(channels, eps = 1e-5, momentum = 0.1, affine = true)` | running statistics update in `train()` mode |
| `BatchNorm1d` | `BatchNorm1d(channels, ...)` | reshapes `(N, C)` for the 2-D kernel |
| `LayerNorm` | `LayerNorm(size, eps = 1e-5, affine = true)` | normalizes the last axis |
| `GroupNorm` | `GroupNorm(groups, channels, eps = 1e-5, affine = true)` | normalizes each channel group over its spatial positions |
| `Embedding` | `Embedding(rows, dimension)` | `forward(indices)` expects integer-valued `Variable`s |
| `Dropout` | `Dropout(p = 0.5)` | uses the global training flag; scales by `1/(1-p)` |
| `MultiheadAttention` | `MultiheadAttention(embedDim, heads, bias = true)` | self-attention only, no mask |
| `TransformerEncoderLayer` | `(embedDim, heads, feedforwardDim = 4*embedDim, dropout = 0.1)` | pre-norm, batch-first `(batch, time, features)` |
| `TransformerEncoder` | `TransformerEncoder(layers)` | a stack of encoder layers |
| `LSTM`, `GRU`, `RNN` | `(inputSize, hiddenSize, bias = true)` | one layer, sequence-first `(steps, batch, features)` |
| `LSTMCell`, `GRUCell`, `RNNCell` | `(inputSize, hiddenSize, bias = true)` | single-step `step(...)` |
| `Sequential` | `Sequential(layers)` | runs layers in order |
| `MLP` | `MLP(inputs, hidden, outputs, activation = box(ReLU()))` | two linear layers and one activation |
| `ModuleList` | `ModuleList(layers)` | same run order, for explicit use |
| `Identity`, `Flatten` | `Identity()`, `Flatten(startDim = 1)` | |
| Activations | `ReLU`, `LeakyReLU(slope = 0.01)`, `ELU(alpha = 1.0)`, `GELU`, `SiLU`, `Sigmoid`, `Tanh`, `Softmax(axis = -1)`, `LogSoftmax(axis = -1)` | |

Functional equivalents live in `std.kesnet`: `relu`, `sigmoid`, `tanh`, `gelu`,
`silu`, `softmax`, `logSoftmax`, `linear`, `mseLoss`, `l1Loss`, `crossEntropy`,
`nllLoss`, `binaryCrossEntropy`, `accuracy`, `meanAbsoluteError`, and
`meanSquaredError`.

Initialization helpers are `xavierUniform`, `kaimingUniform`, `normalInit`,
`zerosInit`, and `onesInit`; each writes into an existing parameter with
`copy_`.

## Losses

`MSELoss`, `L1Loss`, `CrossEntropyLoss`, `NLLLoss`, and `BCEWithLogitsLoss` are
small classes with `forward(prediction, target)`. Cross entropy expects logits
shaped `(rows, classes)` and integer-valued targets shaped `(rows,)` or
`(rows, 1)`. The reverse pass is fused: softmax and the negative log-likelihood
gradient are computed together.

## Optimizers and schedules

`std.kesnet.optim` contains `SGD` (momentum, Nesterov, weight decay), `Adam`,
`AdamW`, `RMSprop`, `Adagrad`, `Adadelta`, and `Adamax`. Every optimizer takes a
parameter list and a learning rate:

```kestrel
let mut optimizer := SGD(model.parameters(), 0.01, momentum = 0.9, nesterov = true)
optimizer.zeroGrad()
loss.backward()
optimizer.step()
```

`zeroGrad` clears every parameter; `setLearningRate` changes the rate; `step`
updates parameters in place. The optimizer keeps per-parameter state in
`Variable`s, so the state follows the same copy and lifetime rules as weights.

Schedules are `StepLR`, `MultiStepLR`, `ExponentialLR`, `CosineAnnealingLR`, and
`ReduceLROnPlateau`; `warmupRate` computes a linear warmup. A schedule takes the
optimizer as a `mut` argument:

```kestrel
let mut schedule := StepLR(100, 0.5)
schedule.step(optimizer)
```

## Data

```kestrel
import std.kesdas.{DataFrame}
import std.kesnet.{Linear, mseLoss}
import std.kesnet.data.{fromDataFrame, DataLoader}
import std.kesnet.optim.{Adam}

function train(frame : DataFrame) : void or Error with Effects {
    let dataset := fromDataFrame(frame, ["x1", "x2"], "y")?
    let mut loader := DataLoader(dataset, batchSize = 32, shuffle = true, seed = 7)
    for (batch in loader.batches()) {
        let (inputs, targets) := batch
        let loss := mseLoss(model.forward(inputs), targets)
        optimizer.zeroGrad()
        loss.backward()
        optimizer.step()
    }
}
```

`TensorDataset` stores feature and target `DynTensor`s. `DataLoader` returns an
`Array` of `(Variable, Variable)` batches; `batches()` advances a seeded epoch,
so a run is reproducible. `dropLast`, batch size, shuffle, and seed are
constructor arguments. `trainTestSplit` holds out a seeded slice.

Because a Kestrel class owning a dataset moves it, read the held-out data through
`loader.dataset` after constructing a loader, or split after evaluation.

## Serialization

`Variable.save(path)` writes a small binary file (magic, version, dtype,
`requiresGrad`, shape, raw doubles). `Variable.load(path)` reads it back, and
`target.copyFromFile(path)` copies the values into an existing leaf so an
optimizer keeps the same node.

`saveParameters(model, path)` and `loadParameters(model, path)` write and read the
whole positional state in one call:

```kestrel
saveParameters(model, "checkpoints/run1")
loadParameters(restored, "checkpoints/run1")
```

The ordering is `module.parameters()`. It is stable as long as the model's
registration order is stable.

## Editor support

The VS Code extension and the Jupyter kernel get their KesNet table from
`tools/export_kesnet_metadata.js`, generated from the library sources:

```sh
node tools/export_kesnet_metadata.js
node tools/export_builtins_metadata.js
```

The table gives completion after `Variable.`, `Linear.`, `Sequential.` and any
local class extending `Module` (inherited `parameters`, `zeroGrad`, `train`,
`eval`, and `summary` included). It also drives inferred binding types,
inlay hints, hover, and signature help, including chains such as
`model.forward(x).relu().sum()`. `editors/vscode-kestrel/test/kesnet.test.js`
and `test/kesnet-metadata.test.js` check the behavior and the table's coverage.

## Design

The engine is C, emitted with the rest of the program by the Kestrel native
backend. A `Variable` is a counted handle to a node. A node holds:

- row-major `double` storage, a rank, a shape, and a dtype;
- an optional gradient buffer;
- a `requires_grad` flag, a leaf flag, a retain-grad flag, and a version;
- an operation record with strong references to its parents and the inputs it
  needs for the reverse pass.

The reverse pass performs a topological sort, seeds the root, walks the order in
reverse, and accumulates vector-Jacobian products. Broadcasting remembers the
output shape and reduces gradients back to each input. Matrix products,
convolution, pooling, normalization, embedding, and cross entropy have dedicated
C kernels; the remaining elementwise and shape operations share small loops.

Memory is reference counted rather than garbage collected. Operations retain
their parents, and arrays and `List`s that hold `Variable`s copy the handle. The
graph is freed when no handle reaches it. `Variable.liveNodes()` is the test hook
for this property.

The language integration is part of the design:

- `Variable` is a built-in type, so it can be a final field, a list item, or a
  tuple member like `String` or `List`.
- Arithmetic, indexing and `@` are checked by the compiler; the methods are in
  the same table the editor reads.
- The compiler records each operator as a builtin call, and the native emitter
  maps it to the C kernel. No Python, FFI shim, or stringly protocol sits between
  the model and the kernel.
- Error messages are checked at compile time for shape-taking calls where the
  rank is literal; the dynamic shape is checked by the engine.

## Limits

KesNet is CPU-only in this build. The `Device` surface exists, but `to("mps")`
and `to("cuda")` stop with a message rather than silently running on the CPU.
There are no Metal or CUDA kernels.

The engine keeps `double` storage even for `float32` values and rounds after each
operation; it models the precision, not the memory saving. There is no mixed
precision autocast.

Not implemented: higher-order gradients (`create_graph`), in-place autograd with
version checks beyond leaf updates, sparse tensors, FFTs, `ConvTranspose2d`,
`Conv1d`, multi-layer or bidirectional RNNs, attention masks,
`DataLoader` worker processes, distributed training, a JIT, and quantization.

`@` handles rank 1 and 2 only; `bmm` handles rank 3. Batched matrix products for
arbitrary leading dimensions are written by reshaping to rank 3.

## Examples

- [`examples/kesnet.kes`](examples/kesnet.kes) — regression with a two-layer
  network, Adam, and a learning-rate schedule.
- [`examples/kesnetcnn.kes`](examples/kesnetcnn.kes) — convolutional classifier
  with cross entropy.
- [`examples/kesnettransformer.kes`](examples/kesnettransformer.kes) — a
  transformer encoder trained on a tiny sequence task.
- [`examples/kesnetrnn.kes`](examples/kesnetrnn.kes) — an LSTM sequence
  regressor.
- [`examples/kesnetdata.kes`](examples/kesnetdata.kes) — a `kesdas` DataFrame,
  `fromDataFrame`, and batched training.
- [`examples/kesnetstate.kes`](examples/kesnetstate.kes) — saving and loading a
  model's parameters.

Run one with:

```sh
kestrel run examples/kesnet.kes
```

The native tests are in [`tests/kesnet.rs`](tests/kesnet.rs); they compile and
execute real programs through the same backend a user invokes.
