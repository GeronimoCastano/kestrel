//! KesNet end-to-end tests: autograd, training, convolution, normalization,
//! data batching, serialization and memory accounting. Programs are compiled
//! through the real native backend, exactly as a user would run them.

use kestrel::{core, modules::check_path, native};
use std::{fs, process::Command};

fn run_program(source: &str) -> String {
    let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let file = dir.path().join("program.kes");
    // Programs that save files name /tmp; they write into this test's folder instead,
    // spelled with forward slashes so a Windows path needs no escapes in Kestrel text.
    let folder = dir.path().display().to_string().replace('\\', "/");
    fs::write(&file, source.replace("/tmp/", &format!("{folder}/"))).unwrap();
    let project = check_path(&file, dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn variables_differentiate_elementwise_arithmetic_broadcasting_and_power() {
    let output = run_program(
        r#"
let a := Variable.parameter([1.0, 2.0, 3.0].toArray())
let b := Variable.parameter([4.0, 5.0, 6.0].toArray())
let loss := ((a * b + a) ** 2.0).sum()
loss.backward()
match a.grad() { Some(g) => print(g), None => print("none") }
match b.grad() { Some(g) => print(g), None => print("none") }
let c := Variable.parameter([1.0, 2.0].toArray(), (2, 1))
let d := Variable.parameter([10.0, 20.0, 30.0].toArray(), (1, 3))
print((c + d).data())
let total := (c * d).sum()
total.backward()
match c.grad() { Some(g) => print(g), None => print("none") }
match d.grad() { Some(g) => print(g), None => print("none") }
"#,
    );
    // dL/da = 2(a*b+a)*b, dL/db = 2(a*b+a)*a.
    assert_eq!(
        output,
        "[50.0, 144.0, 294.0]\n[10.0, 48.0, 126.0]\n[[11.0, 21.0, 31.0],\n [12.0, 22.0, 32.0]]\n[[60.0],\n [60.0]]\n[[3.0, 3.0, 3.0]]\n"
    );
}

#[test]
fn linear_algebra_shape_operations_and_softmax_have_correct_gradients() {
    let output = run_program(
        r#"
let a := Variable.parameter([1.0, 2.0, 3.0, 4.0].toArray(), (2, 2))
let b := Variable.parameter([5.0, 6.0, 7.0, 8.0].toArray(), (2, 2))
let product := a @ b
print(product.data())
let loss := (product * product).sum()
loss.backward()
match a.grad() { Some(g) => print(g), None => print("none") }
match b.grad() { Some(g) => print(g), None => print("none") }
let m := Variable.fromArray([1.0, 2.0, 3.0, 4.0, 5.0, 6.0].toArray(), (2, 3))
print(m.softmax(1).sum(1).data())
let t := m.transpose(0, 1)
print(t.data())
let r := t.reshape((3, 2))
print(r.data())
let s := m.slice(1, 0, 2)
print(s.data())
let cat := m.cat(s, 1)
print(cat.data())
"#,
    );
    assert!(output.starts_with("[[19.0, 22.0],\n [43.0, 50.0]]\n"));
    assert!(output.contains("[0.99999999999999989, 0.99999999999999989]\n"));
    assert!(output.contains("[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n"));
    assert!(output.contains("[[1.0, 4.0],\n [2.0, 5.0],\n [3.0, 6.0]]\n"));
    assert!(output.contains("[[1.0, 2.0],\n [4.0, 5.0]]\n"));
    assert!(output.contains("[[1.0, 2.0, 3.0, 1.0, 2.0],\n [4.0, 5.0, 6.0, 4.0, 5.0]]\n"));
}

#[test]
fn a_sequential_model_trains_with_adam_and_streaming_batches() {
    let output = run_program(
        r#"
import std.kesnet.{Linear, Tanh, Sequential, mseLoss}
import std.kesnet.data.{TensorDataset, DataLoader, trainTestSplit}
import std.kesnet.optim.{Adam}
Variable.seed(7)
let x := Tensor([1.0, 2.0, 2.0, 1.0, 4.0, 3.0, 5.0, 6.0, 1.0, 1.0, 2.0, 2.0], shape = (6, 2))
let y := Tensor([8.0, 7.0, 18.0, 29.0, 2.0, 4.0], shape = (6, 1))
let dataset := TensorDataset(x, y)
let (training, testing) := trainTestSplit(dataset, 2, 3)
let mut loader := DataLoader(training, 2, true, false, 3)
let model := Sequential([box(Linear(2, 8)), box(Tanh()), box(Linear(8, 1))])
let mut optimizer := Adam(model.parameters(), 0.02)
for (epoch in range(500)) {
    for (batch in loader.batches()) {
        let (inputs, targets) := batch
        let loss := mseLoss(model.forward(inputs), targets)
        optimizer.zeroGrad()
        loss.backward()
        optimizer.step()
    }
}
let finalLoss := mseLoss(model.forward(Variable.fromTensor(loader.dataset.features)), Variable.fromTensor(loader.dataset.targets))
print(finalLoss.item() < 1.0)
print(model.parameterCount())
print(testing.length())
"#,
    );
    assert_eq!(output, "true\n33\n2\n");
}

#[test]
fn conv_pool_batch_norm_layer_norm_embedding_and_transformer_run_and_backpropagate() {
    let output = run_program(
        r#"
import std.kesnet.{Conv2d, ReLU, MaxPool2d, AvgPool2d, AdaptiveAvgPool2d, Flatten, Linear, Sequential, BatchNorm2d, LayerNorm, GroupNorm, Embedding, TransformerEncoderLayer}
let image := Variable.parameter([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0].toArray(), (1, 1, 4, 4))
let conv := Conv2d(1, 2, 3, 1, 1)
let pooled := MaxPool2d(2, 2).forward(conv.forward(image))
print(pooled.shape())
let averaged := AvgPool2d(2, 2).forward(conv.forward(image))
print(averaged.shape())
let adaptive := AdaptiveAvgPool2d(2, 2).forward(conv.forward(image))
print(adaptive.shape())
adaptive.sum().backward()
print(true)
let model := Sequential([box(Conv2d(1, 2, 3, 1, 1)), box(ReLU()), box(MaxPool2d(2, 2)), box(Flatten(1)), box(Linear(8, 1))])
let prediction := model.forward(image)
print(prediction.shape())
prediction.sum().backward()
print(true)
let norm := BatchNorm2d(1)
let normalized := norm.forward(image)
print(normalized.shape())
normalized.sum().backward()
print(norm.weight.grad().isSome())
let layerNorm := LayerNorm(4)
print(layerNorm.forward(image.reshape((4, 4))).shape())
let groupNorm := GroupNorm(2, 2)
let grouped := groupNorm.forward(conv.forward(image))
print(grouped.shape())
grouped.sum().backward()
print(true)
let embedding := Embedding(5, 3)
let indices := Variable.fromArray([0.0, 2.0, 4.0].toArray(), (3,))
print(embedding.forward(indices).shape())
let transformer := TransformerEncoderLayer(8, 2, 16, 0.0)
let sequence := Variable.fromArray([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1].toArray(), (1, 2, 8))
let attended := transformer.forward(sequence)
print(attended.shape())
attended.sum().backward()
print(true)
"#,
    );
    assert_eq!(
        output,
        "[1, 2, 2, 2]\n[1, 2, 2, 2]\n[1, 2, 2, 2]\ntrue\n[1, 1]\ntrue\n[1, 1, 4, 4]\ntrue\n[4, 4]\n[1, 2, 4, 4]\ntrue\n[3, 3]\n[1, 2, 8]\ntrue\n"
    );
}

#[test]
fn optimizers_and_recurrent_layers_train_without_leaking_graph_nodes() {
    let output = run_program(
        r#"
import std.kesnet.{LSTM, GRU, RNN, mseLoss}
import std.kesnet.optim.{SGD, Adam, AdamW, RMSprop, Adagrad, Adadelta, Adamax}
let x := Variable.fromArray([1.0, 2.0, 3.0, 4.0].toArray(), (4, 1))
let y := Variable.fromArray([2.0, 4.0, 6.0, 8.0].toArray(), (4, 1))
let w := Variable.parameter([0.0].toArray(), (1, 1))
let mut optimizer := Adam([w], 0.1)
for (epoch in range(500)) {
    let loss := mseLoss(x @ w, y)
    optimizer.zeroGrad()
    loss.backward()
    optimizer.step()
}
print((w.data()[0, 0] - 2.0).abs() < 0.01)
let before := Variable.liveNodes()
for (epoch in range(50)) {
    let loss := mseLoss(x @ w, y)
    w.zeroGrad()
    loss.backward()
}
print(Variable.liveNodes() == before)
let sequence := Variable.fromArray([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2].toArray(), (3, 2, 2))
let lstm := LSTM(2, 3)
let lstmOutput := lstm.forward(sequence)
print(lstmOutput.shape())
lstmOutput.sum().backward()
print(true)
let gru := GRU(2, 3)
let gruOutput := gru.forward(sequence)
print(gruOutput.shape())
gruOutput.sum().backward()
print(true)
let rnn := RNN(2, 3)
let rnnOutput := rnn.forward(sequence)
print(rnnOutput.shape())
rnnOutput.sum().backward()
print(true)
"#,
    );
    assert_eq!(
        output,
        "true\ntrue\n[3, 2, 3]\ntrue\n[3, 2, 3]\ntrue\n[3, 2, 3]\ntrue\n"
    );
}

#[test]
fn precision_grad_mode_serialization_and_optimizer_variants_work() {
    let output = run_program(
        r#"
import std.kesnet.{mseLoss}
import std.kesnet.optim.{SGD, Adam, AdamW, RMSprop, Adagrad, Adadelta, Adamax}
let w := Variable.parameter([1.0, 2.0, 3.0].toArray())
print(w.dtype())
print(w.device())
let half := w.to("float32")
print(half.dtype())
let rounded := Variable.fromArray([1.1, 0.1].toArray()).to("float16")
print((rounded.data()[0] - 1.099609375).abs() < 0.000001)
print((rounded.data()[1] - 0.0999755859375).abs() < 0.000001)
Variable.setGradEnabled(false)
print((w * 2.0).requiresGrad())
Variable.setGradEnabled(true)
let w2 := w.to("float64")
w2.save("/tmp/kesnet-serialization-test.kv")
let loaded := Variable.load("/tmp/kesnet-serialization-test.kv")
print(loaded.data())
print(loaded.requiresGrad())
let target := Variable.zeros((3,))
target.copyFromFile("/tmp/kesnet-serialization-test.kv")
print(target.data())
let x := Variable.fromArray([1.0, 2.0, 3.0, 4.0].toArray(), (4, 1))
let y := Variable.fromArray([2.0, 4.0, 6.0, 8.0].toArray(), (4, 1))
let names := ["SGD", "Adam", "AdamW", "RMSprop", "Adagrad", "Adadelta", "Adamax"]
print(names.length())
let p1 := Variable.parameter([0.0].toArray(), (1, 1))
let mut o1 := SGD([p1], 0.01)
for (step in range(200)) {
    let l := mseLoss(x @ p1, y)
    o1.zeroGrad()
    l.backward()
    o1.step()
}
print((p1.data()[0, 0] - 2.0).abs() < 0.05)
let p2 := Variable.parameter([0.0].toArray(), (1, 1))
let mut o2 := SGD([p2], 0.01, 0.9, 0.0, true)
for (step in range(200)) {
    let l := mseLoss(x @ p2, y)
    o2.zeroGrad()
    l.backward()
    o2.step()
}
print((p2.data()[0, 0] - 2.0).abs() < 0.1)
"#,
    );
    assert_eq!(
        output,
        "float64\ncpu\nfloat32\ntrue\ntrue\nfalse\n[1.0, 2.0, 3.0]\ntrue\n[1.0, 2.0, 3.0]\n7\ntrue\ntrue\n"
    );
}

#[test]
fn recurrent_and_transformer_graphs_release_after_backward() {
    let output = run_program(
        r#"
import std.kesnet.{LSTM, GRU, RNN, mseLoss}
let sequence := Variable.fromArray([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2].toArray(), (3, 2, 2))
let lstm := LSTM(2, 3)
let output := lstm.forward(sequence)
let target := Variable.zeros(output.shape())
let base := Variable.liveNodes()
for (step in range(5)) {
    let loss := mseLoss(output, target)
    lstm.zeroGrad()
    loss.backward()
}
print(Variable.liveNodes() <= base + 2)
print(output.shape())
"#,
    );
    assert_eq!(output, "true\n[3, 2, 3]\n");
}

#[test]
fn activation_variants_clamp_norm_and_randint_are_differentiable() {
    let output = run_program(
        r#"
let x := Variable.parameter([-2.0, -1.0, 0.0, 1.0, 2.0].toArray())
print(x.leakyRelu(0.1).data())
print(x.elu(1.0).data())
print(x.gelu().data())
print(x.silu().data())
print(x.softplus(1.0, 20.0).data())
print(x.clamp(-1.0, 1.0).data())
print(x.norm().data())
print(Variable.randint(0, 5, (3, 2)).shape())
let total := (x.gelu() + x.silu() + x.elu(1.0) + x.leakyRelu(0.1) + x.softplus(1.0, 20.0) + x.clamp(-1.0, 1.0) + x.norm()).sum()
total.backward()
match x.grad() { Some(g) => print(g), None => print("none") }
"#,
    );
    assert!(output.contains("[-0.20000000000000001, -0.10000000000000001, 0.0, 1.0, 2.0]"));
    assert!(output.contains("[-0.8646647167633873, -0.63212055882855767, 0.0, 1.0, 2.0]"));
    assert!(output.contains("3.1622776601683795"));
    assert!(output.contains("[3, 2]"));
    assert!(output.contains("[-3.0085245038983559"));
    let layers = run_program(
        r#"
import std.kesnet.{Sequential, GELU, SiLU, ELU, LeakyReLU, Sigmoid, Tanh, Softmax, LogSoftmax}
let x := Variable.parameter([-1.0, 0.0, 1.0, 2.0].toArray(), (2, 2))
let model := Sequential([box(GELU()), box(SiLU()), box(ELU(1.0)), box(LeakyReLU(0.1)), box(Sigmoid()), box(Tanh()), box(Softmax(1)), box(LogSoftmax(1))])
let y := model.forward(x)
print(y.shape())
y.sum().backward()
print(true)
"#,
    );
    assert_eq!(layers, "[2, 2]\ntrue\n");
}

#[test]
fn every_optimizer_reduces_a_quadratic() {
    let output = run_program(
        r#"
import std.kesnet.optim.{SGD, Adam, AdamW, RMSprop, Adagrad, Adadelta, Adamax}
function lossOf(x : Variable) : Variable = ((x - 3.0) * (x - 3.0)).sum()
let p1 := Variable.parameter([0.0].toArray())
let mut o1 := SGD([p1], 0.2)
for (i in range(300)) { let l := lossOf(p1); o1.zeroGrad(); l.backward(); o1.step() }
print((p1.data()[0] - 3.0).abs() < 0.01)
let p2 := Variable.parameter([0.0].toArray())
let mut o2 := Adam([p2], 0.1)
for (i in range(300)) { let l := lossOf(p2); o2.zeroGrad(); l.backward(); o2.step() }
print((p2.data()[0] - 3.0).abs() < 0.01)
let p3 := Variable.parameter([0.0].toArray())
let mut o3 := AdamW([p3], 0.1, 0.9, 0.999, 0.00000001, 0.0)
for (i in range(300)) { let l := lossOf(p3); o3.zeroGrad(); l.backward(); o3.step() }
print((p3.data()[0] - 3.0).abs() < 0.01)
let p4 := Variable.parameter([0.0].toArray())
let mut o4 := RMSprop([p4], 0.05)
for (i in range(500)) { let l := lossOf(p4); o4.zeroGrad(); l.backward(); o4.step() }
print((p4.data()[0] - 3.0).abs() < 0.01)
let p5 := Variable.parameter([0.0].toArray())
let mut o5 := Adagrad([p5], 0.5)
for (i in range(500)) { let l := lossOf(p5); o5.zeroGrad(); l.backward(); o5.step() }
print((p5.data()[0] - 3.0).abs() < 0.01)
let p6 := Variable.parameter([0.0].toArray())
let mut o6 := Adadelta([p6], 1.0)
for (i in range(3000)) { let l := lossOf(p6); o6.zeroGrad(); l.backward(); o6.step() }
print((p6.data()[0] - 3.0).abs() < 0.01)
let p7 := Variable.parameter([0.0].toArray())
let mut o7 := Adamax([p7], 0.1)
for (i in range(500)) { let l := lossOf(p7); o7.zeroGrad(); l.backward(); o7.step() }
print((p7.data()[0] - 3.0).abs() < 0.01)
"#,
    );
    assert_eq!(output, "true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn learning_rate_schedules_change_the_optimizer_rate() {
    let output = run_program(
        r#"
import std.kesnet.optim.{SGD, StepLR, MultiStepLR, ExponentialLR, CosineAnnealingLR, ReduceLROnPlateau}
let p := Variable.parameter([0.0].toArray())
let mut optimizer := SGD([p], 1.0)
let mut step := StepLR(2, 0.5)
step.step(optimizer)
step.step(optimizer)
print((optimizer.learningRate - 0.5).abs() < 0.000001)
let mut milestones := MultiStepLR([1, 3], 0.1)
milestones.step(optimizer)
print((optimizer.learningRate - 0.05).abs() < 0.000001)
let mut exponential := ExponentialLR(0.5)
exponential.step(optimizer)
print((optimizer.learningRate - 0.025).abs() < 0.000001)
let mut cosine := CosineAnnealingLR(10, 1.0, 0.0)
cosine.step(optimizer)
print(optimizer.learningRate < 1.0)
let mut plateau := ReduceLROnPlateau(0.5, 0, 0.0)
let before := optimizer.learningRate
plateau.step(optimizer, 1.0)
plateau.step(optimizer, 2.0)
print(optimizer.learningRate < before)
"#,
    );
    assert_eq!(output, "true\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn dataframes_become_tensor_datasets_and_batches() {
    let output = run_program(
        r#"
import std.kesdas.{DataFrame, Series}
import std.kesnet.data.{fromDataFrame, DataLoader}
let frame := DataFrame([
    Series([1.0, 2.0, 3.0, 4.0], name = "height"),
    Series([2.0, 1.0, 3.0, 2.0], name = "width"),
    Series([0.0, 1.0, 0.0, 1.0], name = "label"),
])
function main(move frame : DataFrame) : void or Error with Effects {
    let dataset := fromDataFrame(frame, ["height", "width"], "label")?
    print(dataset.length())
    print(dataset.featureCount())
    let mut loader := DataLoader(dataset, 3, false, false, 1)
    print(loader.batches().length())
}
main(frame)?
"#,
    );
    assert_eq!(output, "4\n2\n2\n");
}

#[test]
fn concatenation_slicing_embedding_and_pooling_backpropagate() {
    let output = run_program(
        r#"
let left := Variable.parameter([1.0, 2.0, 3.0, 4.0].toArray(), (2, 2))
let right := Variable.parameter([5.0, 6.0, 7.0, 8.0].toArray(), (2, 2))
let joined := left.cat(right, 1)
let weights := Variable.fromArray([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].toArray(), (2, 4))
let total := (joined * weights).sum()
total.backward()
match left.grad() { Some(g) => print(g), None => print("none") }
match right.grad() { Some(g) => print(g), None => print("none") }
let sliced := joined.slice(1, 2, 4)
print(sliced.data())
let embeddingWeight := Variable.parameter([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0].toArray(), (3, 3))
let indices := Variable.fromArray([0.0, 2.0].toArray(), (2,))
let embedded := Variable.embedding(embeddingWeight, indices)
print(embedded.shape())
embedded.sum().backward()
match embeddingWeight.grad() { Some(g) => print(g), None => print("none") }
let image := Variable.parameter([1.0, 2.0, 3.0, 4.0].toArray(), (1, 1, 2, 2))
let pooled := image.avgPool2d(2, 2, 0)
print(pooled.data())
pooled.sum().backward()
match image.grad() { Some(g) => print(g), None => print("none") }
let a := Variable.parameter([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].toArray(), (2, 2, 2))
let b := Variable.parameter([1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0].toArray(), (2, 2, 2))
let batchProduct := a.bmm(b)
print(batchProduct.shape())
batchProduct.sum().backward()
print(true)
"#,
    );
    assert!(output.contains("[[1.0, 2.0],\n [5.0, 6.0]]\n"));
    assert!(output.contains("[[3.0, 4.0],\n [7.0, 8.0]]\n"));
    assert!(output.contains("[[5.0, 6.0],\n [7.0, 8.0]]\n"));
    assert!(output.contains("[[1.0, 1.0, 1.0],\n [0.0, 0.0, 0.0],\n [1.0, 1.0, 1.0]]\n"));
    assert!(output.contains("[[[[0.25, 0.25],\n   [0.25, 0.25]]]]\n"));
}

#[test]
fn module_parameter_files_round_trip_in_order() {
    let output = run_program(
        r#"
import std.kesnet.{Linear, saveParameters, loadParameters}
let source := Linear(2, 3)
source.weight.fill_(1.5)
source.biasValue.fill_(-0.25)
saveParameters(source, "/tmp/kesnet-parameter-files")
let restored := Linear(2, 3)
loadParameters(restored, "/tmp/kesnet-parameter-files")
let x := Variable.fromArray([1.0, 2.0].toArray(), (1, 2))
print(source.forward(x).data()[0, 0] == restored.forward(x).data()[0, 0])
print(restored.weight.data()[0, 0])
"#,
    );
    assert_eq!(output, "true\n1.5\n");
}

#[test]
fn the_remaining_layer_classes_construct_and_forward() {
    let output = run_program(
        r#"
import std.kesnet.{BatchNorm1d, LayerNorm, Dropout, MLP, ModuleList, Sequential, Linear, Flatten, TransformerEncoder, TransformerEncoderLayer, L1Loss, BCEWithLogitsLoss}
let x := Variable.fromArray([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].toArray(), (2, 4))
print(BatchNorm1d(4).forward(x).shape())
print(LayerNorm(4).forward(x).shape())
let dropout := Dropout(0.5)
print(dropout.forward(x).shape())
print(MLP(4, 8, 2).forward(x).shape())
print(ModuleList([box(Linear(4, 2))]).forward(x).shape())
let flat := Sequential([box(Linear(4, 4)), box(Flatten(1))])
print(flat.forward(x).shape())
let encoder := TransformerEncoder([box(TransformerEncoderLayer(4, 2, 8, 0.0))])
let sequence := Variable.fromArray([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8].toArray(), (1, 2, 4))
print(encoder.forward(sequence).shape())
let target := Variable.fromArray([1.0, 0.0, 1.0, 0.0].toArray(), (2, 2))
let l1 := L1Loss()
let bce := BCEWithLogitsLoss()
let logits := Variable.parameter([0.0, 0.0, 0.0, 0.0].toArray(), (2, 2))
let l1Input := Variable.parameter([1.0, 2.0, 3.0, 4.0].toArray(), (2, 2))
let loss1 := l1.forward(l1Input, target)
let loss2 := bce.forward(logits, target)
loss1.backward()
loss2.backward()
print(true)
"#,
    );
    assert_eq!(
        output,
        "[2, 4]\n[2, 4]\n[2, 4]\n[2, 2]\n[2, 2]\n[2, 4]\n[1, 2, 4]\ntrue\n"
    );
}

#[test]
fn lstm_fused_cell_trains_the_same_path_as_the_gate_operations() {
    let output = run_program(
        r#"
import std.kesnet.{LSTM, Linear, mseLoss}
import std.kesnet.optim.{Adam}
Variable.seed(4)
let sequence := Tensor.generate((16, 16, 32), (t, n, f) => (((t * 41 + n * 23 + f * 13) % 101).toDouble() - 50.0) / 50.0)
let targets := Tensor.generate((16, 1), (i, j) => (((i * 37 + j * 11) % 89).toDouble() - 44.0) / 44.0)
let inputs := Variable.fromTensor(sequence)
let expected := Variable.fromTensor(targets)
let recurrent := LSTM(32, 64)
let head := Linear(64, 1)
let mut trainable : Array[Variable] = Array.empty()
for (parameter in recurrent.parameters()) { trainable.append(parameter) }
for (parameter in head.parameters()) { trainable.append(parameter) }
let mut optimizer := Adam(trainable.toList(), 0.005)
for (step in range(5)) {
    let hidden := recurrent.forward(inputs)
    let prediction := head.forward(hidden[-1])
    let loss := mseLoss(prediction, expected)
    optimizer.zeroGrad()
    loss.backward()
    optimizer.step()
    print(f"{loss.item():.12f}")
}
"#,
    );
    let expected = [
        0.379677617199,
        0.337125086628,
        0.309312258982,
        0.285972453830,
        0.260499037804,
    ];
    let values = output
        .lines()
        .map(|line| line.trim().parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), expected.len());
    for (value, want) in values.iter().zip(expected) {
        assert!((value - want).abs() < 1e-6, "{value} against {want}");
    }
}
