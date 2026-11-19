// Checking for KesNet's native `Variable` type.
//
// A Variable is a counted handle to a native autograd node. Unlike Tensor it
// has no shape in the type: dimensions are a run-time property, which is what
// training loops need (batch sizes and convolution geometry vary by input).
// Every method below names a builtin that `native_variable.rs` lowers to the
// C engine in `autograd_runtime.c`.

impl Checker<'_> {
    /// A call whose callee is a member of a Variable, or a static factory such
    /// as `Variable.zeros(...)`. Returns None for names the generic member path
    /// should report.
    fn variable_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        use crate::types::{is_variable, tensor_type, Type};
        let callee = ungroup(node.children.first()?);
        if callee.kind != K::Member {
            return None;
        }
        let name = self.text(&callee.children[1]).to_owned();
        let static_spec = variable_static(&name);
        let method_spec = variable_method(&name);
        if static_spec.is_none() && method_spec.is_none() {
            return None;
        }
        let previous_receiver = self
            .member_receiver
            .replace(ungroup(&callee.children[0]).span);
        let receiver = self.infer(&callee.children[0], None);
        self.member_receiver = previous_receiver;
        let receiver = self.inference.resolve(&receiver);
        let static_call = matches!(&receiver, Type::Constructor(owner, _) if owner == "Variable");
        if static_call && static_spec.is_none() || !static_call && !is_variable(&receiver) {
            return None;
        }
        if !static_call && method_spec.is_none() {
            return None;
        }
        let raw = node
            .children
            .iter()
            .skip(1)
            .map(|argument| {
                (
                    (argument.kind == K::NamedArgument)
                        .then(|| self.text(&argument.children[0]).to_owned()),
                    Type::Invalid,
                    argument.children.last().unwrap(),
                    false,
                )
            })
            .collect::<Vec<_>>();
        let spec = if static_call {
            static_spec.unwrap()
        } else {
            method_spec.unwrap()
        };
        let labels = if static_call {
            variable_static_labels(&name, raw.len())
        } else {
            variable_labels(&name, raw.len())
        };
        let Some(labels) = labels else {
            self.error(
                node,
                "E_CALL_ARGUMENTS",
                format!(
                    "Variable.{name} does not take {} argument{}.",
                    raw.len(),
                    if raw.len() == 1 { "" } else { "s" }
                ),
                spec.hint(),
            );
            return Some(Type::Invalid);
        };
        let Type::Function(mut skeleton) =
            Type::function(vec![Type::Invalid; labels.len()], Type::Invalid)
        else {
            unreachable!()
        };
        for (parameter, label) in skeleton.parameters.iter_mut().zip(&labels) {
            parameter.name = label.map(|name| name.to_owned());
        }
        let Some(order) = argument_order(&skeleton, &raw) else {
            self.error(
                node,
                "E_CALL_ARGUMENTS",
                format!("Variable.{name} has invalid arguments."),
                "Supply each declared parameter once, using its name or position.",
            );
            return Some(Type::Invalid);
        };
        let ordered = order
            .into_iter()
            .map(|index| &raw[index.unwrap()])
            .collect::<Vec<_>>();
        let inputs = ordered
            .iter()
            .map(|argument| argument.2)
            .collect::<Vec<_>>();
        let mut parameters = vec![];
        let mut arguments = vec![];
        let mut effects = false;
        for (position, input) in inputs.iter().enumerate() {
            let label = labels[position].unwrap_or("");
            let hint = spec.hint_for(label);
            let ty = self.infer(input, hint.as_ref());
            let ty = self.inference.resolve(&ty);
            if let Some(wanted) = &hint
                && !matches!(name.as_str(), "fromArray" | "parameter" | "spline")
            {
                self.require(input, wanted, &ty);
            }
            parameters.push(ty.clone());
            arguments.push((ordered[position].0.clone(), ty, *input, false));
            if matches!(name.as_str(), "backward") {
                effects = false;
            }
        }
        let result = if name == "grad" {
            Type::applied("Option", vec![tensor_type(None)])
        } else if name == "data" || name == "toTensor" {
            tensor_type(None)
        } else if name == "shape" {
            Type::applied("Array", vec![Type::named("Int")])
        } else if matches!(name.as_str(), "rank" | "count") {
            Type::named("Int")
        } else if name == "item" {
            Type::named("double")
        } else if matches!(name.as_str(), "requiresGrad" | "isLeaf") {
            Type::named("Bool")
        } else if matches!(name.as_str(), "text" | "debug" | "dtype" | "device") {
            Type::named("String")
        } else if name == "seed" || name == "backward" || name == "zeroGrad" || name == "setTraining" || name == "setGradEnabled" || name == "save" || name == "copyFromFile" || name == "adamStep" {
            Type::named("void")
        } else if name == "isTraining" || name == "isGradEnabled" {
            Type::named("Bool")
        } else if matches!(name.as_str(), "liveNodes" | "liveBytes") {
            Type::named("Int")
        } else {
            Type::named("Variable")
        };
        if name == "backward" {
            effects = false;
        }
        let Type::Function(mut function) = Type::function(parameters, result.clone()) else {
            unreachable!()
        };
        for (parameter, label) in function.parameters.iter_mut().zip(&labels) {
            parameter.name = label.map(|name| name.to_owned());
        }
        function.callable = Some(Callable::Builtin(format!("std.core.Variable.{name}")));
        function.receiver = (!static_call).then(|| Box::new(receiver));
        function.effects = effects;
        arguments.sort_by_key(|argument| argument.2.span.start);
        Some(self.apply(node, Type::Function(function.clone()), &arguments, expected))
    }

    /// The members that are values rather than calls. Only the transpose alias
    /// is needed today; ordinary methods are recorded by `variable_call`.
    fn variable_member(&mut self, receiver: &Type, name: &str) -> Option<Type> {
        use crate::types::Type;
        if !crate::types::is_variable(receiver) {
            return None;
        }
        match name {
            "T" => Some(Type::named("Variable")),
            _ => None,
        }
    }

    /// Elementwise and matrix operators. Broadcasting is a run-time property,
    /// so any Variable/double pair is accepted here and checked by the engine.
    fn variable_binary(&mut self, node: &Node, op: &str, left: &Type, right: &Type) -> Type {
        use crate::types::{is_variable, Type};
        let l = is_variable(left);
        let r = is_variable(right);
        let scalar = |ty: &Type| {
            matches!(ty, Type::Named(name, args) if args.is_empty() && matches!(name.as_str(), "Int" | "double"))
        };
        if !l && !r {
            return Type::Invalid;
        }
        if op == "@" {
            if !(l && r) {
                self.error(
                    node,
                    "E_VARIABLE_OPERATOR",
                    "`@` needs a Variable on both sides.",
                    "Use `x @ weight`, or `x.matmul(weight)`, with Variables made from tensors.",
                );
                return Type::Invalid;
            }
            self.record_operator(
                node,
                left,
                right,
                Type::named("Variable"),
                ("Variable", "matmul"),
                false,
            );
            return Type::named("Variable");
        }
        if op == "**" {
            if !(l && (r || scalar(right))) {
                self.error(
                    node,
                    "E_VARIABLE_OPERATOR",
                    "`**` needs a Variable base and a Variable or double exponent.",
                    "Use `x ** 2` or `x.power(2)`.",
                );
                return Type::Invalid;
            }
            self.record_operator(
                node,
                left,
                right,
                Type::named("Variable"),
                ("Variable", "power"),
                false,
            );
            return Type::named("Variable");
        }
        let method = match op {
            "+" => ("add", "rightAdd"),
            "-" => ("subtract", "rightSubtract"),
            "*" => ("multiply", "rightMultiply"),
            "/" => ("divide", "rightDivide"),
            _ => {
                self.error(
                    node,
                    "E_VARIABLE_OPERATOR",
                    format!("A Variable does not support `{op}`."),
                    "Use +, -, *, / or @ on Variables, or call the method explicitly.",
                );
                return Type::Invalid;
            }
        };
        let reverse = !l;
        if reverse && !scalar(left) {
            self.error(
                node,
                "E_VARIABLE_OPERATOR",
                format!("`{op}` needs a Variable on the left or a number on the left."),
                "A built-in left operand must be an Int or double; otherwise put the Variable first.",
            );
            return Type::Invalid;
        }
        let receiver = if l { left } else { right };
        let operand = if l { right } else { left };
        if !(scalar(operand) || is_variable(operand)) {
            self.error(
                node,
                "E_VARIABLE_OPERATOR",
                format!("`{op}` needs a Variable or number on the right."),
                "Use a Variable of the same or broadcastable shape, or a double scalar.",
            );
            return Type::Invalid;
        }
        if reverse && !matches!(op, "+" | "*") {
            self.record_operator(
                node,
                receiver,
                operand,
                Type::named("Variable"),
                ("Variable", method.1),
                true,
            );
        } else {
            self.record_operator(
                node,
                receiver,
                operand,
                Type::named("Variable"),
                ("Variable", method.0),
                reverse,
            );
        }
        Type::named("Variable")
    }

    /// `x[i]`, `x[:, 0]` and slice indexing. Integer and slice positions both
    /// count as dynamic shape operations; the result is always a Variable.
    fn variable_index(&mut self, node: &Node, variable: &Type) -> Type {
        let _ = variable;
        let parts = match node.children[1].kind {
            K::Tuple => node.children[1].children.iter().collect::<Vec<_>>(),
            _ => vec![&node.children[1]],
        };
        if parts.is_empty() {
            self.error(
                node,
                "E_VARIABLE_INDEX",
                "A Variable index needs at least one position.",
                "Write an integer for one position or a half-open slice such as 1:4.",
            );
            return Type::Invalid;
        }
        for part in parts {
            if part.kind == K::Slice {
                for bound in &part.children {
                    if bound.kind != K::Hole {
                        let ty = self.infer(bound, Some(&Type::named("Int")));
                        self.require(bound, &Type::named("Int"), &ty);
                    }
                }
            } else {
                let ty = self.infer(part, Some(&Type::named("Int")));
                self.require(part, &Type::named("Int"), &ty);
            }
        }
        Type::named("Variable")
    }
}

/// One entry in the Variable method table.
struct VariableSpec {
    hint: fn() -> &'static str,
}

impl VariableSpec {
    fn hint_for(&self, label: &str) -> Option<Type> {
        use crate::types::Type;
        match label {
            "axis" | "dim" | "start" | "stop" | "steps" | "n" | "dim0" | "dim1" => {
                Some(Type::named("Int"))
            }
            "keepdim" | "retainGraph" | "training" | "requiresGrad" | "enabled" => Some(Type::named("Bool")),
            "value" | "min" | "max" | "slope" | "eps" | "p" | "alpha" | "low" | "high"
            | "step" | "beta" | "fill" | "scale" => Some(Type::named("double")),
            "dtype" | "device" | "path" => Some(Type::named("String")),
            "stride" | "padding" | "dilation" | "groups" | "kernel" => Some(Type::named("Int")),
            "epsilon" | "momentum" => Some(Type::named("double")),
            "shape" | "axes" | "source" => None,
            "values" => Some(Type::applied(
                "Array",
                vec![Type::named("double")],
            )),
            _ => None,
        }
    }

    fn hint(&self) -> &'static str {
        (self.hint)()
    }
}

fn variable_static(name: &str) -> Option<VariableSpec> {
    matches!(
        name,
        "zeros"
            | "ones"
            | "filled"
            | "fromArray"
            | "fromTensor"
            | "scalar"
            | "arange"
            | "linspace"
            | "randn"
            | "rand"
            | "randint"
            | "eye"
            | "parameter"
            | "zerosLike"
            | "onesLike"
            | "randLike"
            | "uniform"
            | "normal"
            | "bernoulli"
            | "setTraining"
            | "isTraining"
            | "empty"
            | "embedding"
            | "liveNodes"
            | "liveBytes"
            | "setGradEnabled"
            | "isGradEnabled"
            | "load"
            | "seed"
            | "adamStep"
    )
    .then_some(VariableSpec {
        hint: || "Use a shape tuple or Array[Int], then the values the factory needs.",
    })
}

fn variable_static_labels(name: &str, arity: usize) -> Option<Vec<Option<&'static str>>> {
    let labels: &[Option<&'static str>] = match (name, arity) {
        ("zeros" | "ones" | "randn" | "rand" | "eye", 1) => &[Some("shape")],
        ("zerosLike" | "onesLike" | "randLike", 1) => &[Some("source")],
        ("uniform", 3) => &[Some("shape"), Some("low"), Some("high")],
        ("normal", 3) => &[Some("shape"), Some("mean"), Some("sigma")],
        ("bernoulli", 2) => &[Some("shape"), Some("p")],
        ("setTraining", 1) => &[Some("training")],
        ("isTraining" | "empty" | "liveNodes" | "liveBytes" | "isGradEnabled", 0) => &[],
        ("setGradEnabled", 1) => &[Some("enabled")],
        ("load", 1) => &[Some("path")],
        ("embedding", 2) => &[Some("weight"), Some("indices")],
        ("filled", 2) => &[Some("shape"), Some("value")],
        ("fromArray", 1) => &[Some("values")],
        ("fromArray" | "parameter", 2) => &[Some("values"), Some("shape")],
        ("parameter", 1) => &[Some("values")],
        ("fromTensor", 1) => &[Some("source")],
        ("scalar", 1) => &[Some("value")],
        ("arange", 2) => &[Some("start"), Some("stop")],
        ("arange", 3) => &[Some("start"), Some("stop"), Some("step")],
        ("linspace", 3) => &[Some("start"), Some("stop"), Some("steps")],
        ("randint", 3) => &[Some("low"), Some("high"), Some("shape")],
        ("seed", 1) => &[Some("value")],
        ("adamStep", 11) => &[
            Some("parameter"), Some("gradient"), Some("first"), Some("second"),
            Some("learningRate"), Some("beta1"), Some("beta2"), Some("epsilon"),
            Some("correction1"), Some("correction2"), Some("weightDecay"),
        ],
        _ => return None,
    };
    Some(labels.to_vec())
}

fn variable_method(name: &str) -> Option<VariableSpec> {
    let known = matches!(
        name,
        "shape"
            | "rank"
            | "count"
            | "dtype"
            | "device"
            | "requiresGrad"
            | "requiresGrad_"
            | "isLeaf"
            | "grad"
            | "gradVariable"
            | "data"
            | "toTensor"
            | "item"
            | "copy"
            | "clone"
            | "detach"
            | "text"
            | "debug"
            | "backward"
            | "zeroGrad"
            | "add"
            | "subtract"
            | "multiply"
            | "divide"
            | "rightAdd"
            | "rightSubtract"
            | "rightMultiply"
            | "rightDivide"
            | "matmul"
            | "mm"
            | "bmm"
            | "dot"
            | "power"
            | "negate"
            | "abs"
            | "exp"
            | "expm1"
            | "log"
            | "log1p"
            | "log2"
            | "log10"
            | "sqrt"
            | "rsqrt"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "sigmoid"
            | "relu"
            | "leakyRelu"
            | "elu"
            | "gelu"
            | "silu"
            | "softplus"
            | "softmax"
            | "logSoftmax"
            | "clamp"
            | "sum"
            | "mean"
            | "prod"
            | "max"
            | "min"
            | "argmax"
            | "argmin"
            | "norm"
            | "crossEntropy"
            | "nllLoss"
            | "binaryCrossEntropy"
            | "reshape"
            | "transpose"
            | "permute"
            | "squeeze"
            | "unsqueeze"
            | "flatten"
            | "cat"
            | "slice"
            | "broadcastTo"
            | "expand"
            | "to"
            | "add_"
            | "sub_"
            | "mul_"
            | "div_"
            | "fill_"
            | "zero_"
            | "copy_"
            | "conv2d"
            | "maxPool2d"
            | "avgPool2d"
            | "adaptiveAvgPool2d"
            | "batchNorm"
            | "layerNorm"
            | "save"
            | "copyFromFile"
            | "retainGrad"
            | "lstmCell"
            | "lstmHidden"
            | "lstmForward"
    );
    if !known {
        return None;
    }
    Some(VariableSpec {
        hint: || "Check the arguments of this Variable method.",
    })
}

fn variable_labels(name: &str, arity: usize) -> Option<Vec<Option<&'static str>>> {
    let labels: &[Option<&'static str>] = match (name, arity) {
        ("shape" | "rank" | "count" | "dtype" | "device" | "requiresGrad" | "isLeaf"
        | "grad" | "gradVariable" | "data" | "toTensor" | "item" | "copy" | "clone" | "detach" | "text"
        | "debug" | "zeroGrad" | "negate" | "abs" | "exp" | "expm1" | "log" | "log1p"
        | "log2" | "log10" | "sqrt" | "rsqrt" | "sin" | "cos" | "tan" | "asin" | "acos"
        | "atan" | "sinh" | "cosh" | "tanh" | "sigmoid" | "relu" | "silu" | "gelu"
        | "sum" | "mean" | "prod" | "max" | "min" | "argmax" | "argmin" | "norm"
        | "squeeze" | "flatten" | "add_" | "sub_" | "mul_" | "div_" | "fill_" | "zero_"
        | "copy_", 0) => &[],
        ("isTraining", 0) => &[],
        ("backward", 0) => &[],
        ("backward", 1) => &[Some("retainGraph")],
        ("requiresGrad_", 1) => &[Some("requiresGrad")],
        ("to", 1) => &[Some("dtype")],
        ("save" | "copyFromFile", 1) => &[Some("path")],
        ("retainGrad", 0) => &[],
        ("sum" | "mean" | "max" | "min" | "argmax" | "argmin", 1) => {
            &[Some("axis")]
        }
        ("sum" | "mean" | "max" | "min" | "argmax" | "argmin", 2) => {
            &[Some("axis"), Some("keepdim")]
        }
        ("softmax" | "logSoftmax", 0) => &[],
        ("softmax" | "logSoftmax", 1) => &[Some("axis")],
        ("leakyRelu", 1) => &[Some("slope")],
        ("elu", 1) => &[Some("alpha")],
        ("softplus", 2) => &[Some("beta"), Some("threshold")],
        ("clamp", 2) => &[Some("min"), Some("max")],
        ("crossEntropy" | "nllLoss" | "binaryCrossEntropy", 1) => &[Some("target")],
        ("conv2d", 6) => &[Some("weight"), Some("bias"), Some("stride"), Some("padding"), Some("dilation"), Some("groups")],
        ("maxPool2d" | "avgPool2d", 3) => &[Some("kernel"), Some("stride"), Some("padding")],
        ("adaptiveAvgPool2d", 2) => &[Some("outHeight"), Some("outWidth")],
        ("batchNorm", 7) => &[Some("weight"), Some("bias"), Some("runningMean"), Some("runningVar"), Some("epsilon"), Some("momentum"), Some("training")],
        ("layerNorm", 3) => &[Some("weight"), Some("bias"), Some("epsilon")],
        ("power" | "add" | "subtract" | "multiply" | "divide" | "rightAdd"
        | "rightSubtract" | "rightMultiply" | "rightDivide" | "matmul" | "mm" | "bmm"
        | "dot", 1) => &[Some("other")],
        ("reshape" | "broadcastTo" | "expand" | "permute", 1) => &[Some("shape")],
        ("transpose", 2) => &[Some("dim0"), Some("dim1")],
        ("unsqueeze", 1) => &[Some("dim")],
        ("flatten", 1) => &[Some("startDim")],
        ("cat", 2) => &[Some("other"), Some("axis")],
        ("slice", 3) => &[Some("axis"), Some("start"), Some("end")],
        ("lstmCell" | "lstmHidden", 1) => &[Some("cell")],
        ("lstmForward", 4) => &[Some("weightInput"), Some("biasInput"), Some("weightHidden"), Some("biasHidden")],
        ("add_" | "sub_" | "mul_" | "div_" | "copy_", 1) => &[Some("other")],
        ("fill_", 1) => &[Some("value")],
        _ => return None,
    };
    Some(labels.to_vec())
}
