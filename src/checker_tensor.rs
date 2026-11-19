impl Checker<'_> {
    fn tensor_equal_shape_type(&self, left: &Type, right: &Type) -> Option<Type> {
        let (a, b) = (
            crate::types::tensor_shape(left),
            crate::types::tensor_shape(right),
        );
        if a.is_some() && b.is_some() && !self.inference.clone().unify(left, right) {
            return None;
        }
        Some(crate::types::tensor_type(a.or(b)))
    }

    fn tensor_binary(&mut self, node: &Node, op: &str, left: &Type, right: &Type) -> Type {
        use crate::types::{is_tensor, tensor_shape, tensor_type};
        let (a, b) = (is_tensor(left), is_tensor(right));
        if op == "**" {
            if a && (*right == Type::named("Int") || *right == Type::named("double")) {
                let result = tensor_type(tensor_shape(left));
                self.record_operator(
                    node,
                    left,
                    right,
                    result.clone(),
                    ("Powerable", "power"),
                    false,
                );
                return result;
            }
            self.error(
                node,
                "E_OPERATOR_TRAIT",
                "Tensor `**` needs an Int or double exponent on the right.",
                "Use `A ** 2` for a square matrix power, or `A ** 2.0` for an elementwise power.",
            );
            return Type::Invalid;
        }
        if op == "/" && a && *right == Type::named("double") {
            let result = tensor_type(tensor_shape(left));
            self.record_operator(
                node,
                left,
                right,
                result.clone(),
                ("Divisible", "divide"),
                false,
            );
            return result;
        }
        let operators = match op {
            "*" => Some((
                "Multipliable",
                "multiply",
                "RightMultipliable",
                "rightMultiply",
            )),
            "+" => Some(("Addable", "add", "RightAddable", "rightAdd")),
            "-" => Some((
                "Subtractable",
                "subtract",
                "RightSubtractable",
                "rightSubtract",
            )),
            _ => None,
        };
        if let Some((protocol, method, right_protocol, right_method)) = operators
            && (a && *right == Type::named("double") || b && *left == Type::named("double"))
        {
            let (receiver, operand, protocol, method, reverse) = if a {
                (left, right, protocol, method, false)
            } else {
                (right, left, right_protocol, right_method, true)
            };
            let result = tensor_type(tensor_shape(receiver));
            self.record_operator(
                node,
                receiver,
                operand,
                result.clone(),
                (protocol, method),
                reverse,
            );
            return result;
        }
        if let Some((protocol, method, _, _)) = operators
            && a
            && b
        {
            if let Some(result) = self.tensor_equal_shape_type(left, right) {
                if tensor_shape(left).is_some() && tensor_shape(right).is_some() {
                    self.inference.unify(left, right);
                }
                self.record_operator(
                    node,
                    left,
                    right,
                    result.clone(),
                    (protocol, method),
                    false,
                );
                return self.inference.resolve(&result);
            }
            self.error(
                node,
                "E_TENSOR_SHAPE",
                format!("Elementwise `{op}` requires equal shapes; found {left} and {right}."),
                "Use equal shapes, or broadcastTo explicitly before multiplying.",
            );
            return Type::Invalid;
        }
        if a && b && op == "@" {
            let (a, b) = (tensor_shape(left), tensor_shape(right));
            if a.as_ref().is_some_and(|s| !matches!(s.len(), 1 | 2))
                || b.as_ref().is_some_and(|s| !matches!(s.len(), 1 | 2))
                || a.as_ref().is_some_and(|s| s.len() == 1)
                    && b.as_ref().is_some_and(|s| s.len() == 1)
            {
                self.error(node, "E_TENSOR_RANK", "@ needs a matrix and a vector, or two matrices.", "Use linalg.dot for two vectors; @ supports ranks 1 and 2 without batch broadcasting.");
                return Type::Invalid;
            }
            let result = match (a, b) {
                (Some(a), Some(b)) => {
                    let dim = |i: &Index| Type::Indexed(Box::new(Type::named("Nat")), i.clone());
                    if !self.inference.unify(&dim(a.last().unwrap()), &dim(&b[0])) {
                        self.error(node, "E_TENSOR_SHAPE", "Matrix multiplication has different inner dimensions.", "Match the contracted dimensions; use linalg.outer for an outer product.");
                        return Type::Invalid;
                    }
                    let mut result = vec![];
                    if a.len() == 2 {
                        result.push(a[0].clone());
                    }
                    if b.len() == 2 {
                        result.push(b[1].clone());
                    }
                    tensor_type(Some(result))
                }
                _ => tensor_type(None),
            };
            // @ has no additional public operator trait in Phase 1.
            self.record_operator(
                node,
                left,
                right,
                result.clone(),
                ("Tensor", "matmul"),
                false,
            );
            return self.inference.resolve(&result);
        }
        self.error(
            node,
            "E_TENSOR_OPERATOR",
            format!("Tensors do not support `{op}` with {left} and {right}."),
            "Use equal-shape tensor +, - or *, double scalars with +, - or *, tensor / double, or rank-1/2 tensor @.",
        );
        Type::Invalid
    }

    fn tensor_index(&mut self, node: &Node, tensor: &Type) -> Type {
        use crate::types::{tensor_shape, tensor_type};
        let parts = tensor_indices(&node.children[1]);
        let shape = tensor_shape(tensor);
        if shape
            .as_ref()
            .is_some_and(|shape| shape.len() != parts.len())
        {
            self.error(
                node,
                "E_TENSOR_RANK",
                format!(
                    "This tensor needs {} indices, found {}.",
                    shape.as_ref().unwrap().len(),
                    parts.len()
                ),
                "Supply one integer or slice for each dimension.",
            );
        }
        let mut output = Some(vec![]);
        let mut sliced = false;
        for (position, part) in parts.iter().enumerate() {
            if part.kind != K::Slice {
                let ty = self.infer(part, Some(&Type::named("Int")));
                self.require(part, &Type::named("Int"), &ty);
                continue;
            }
            sliced = true;
            if self.assignment_target_span == Some(node.span) {
                self.error(
                    node,
                    "E_TENSOR_SLICE_ASSIGN",
                    "A tensor slice is a copy and cannot be assigned through.",
                    "Assign individual tensor elements.",
                );
            }
            for bound in &part.children {
                if bound.kind != K::Hole {
                    let ty = self.infer(bound, Some(&Type::named("Int")));
                    self.require(bound, &Type::named("Int"), &ty);
                }
            }
            let low = &part.children[0];
            let high = &part.children[1];
            let dimension = if low.kind == K::Hole && high.kind == K::Hole {
                shape.as_ref().and_then(|s| s.get(position)).cloned()
            } else {
                let lo = if low.kind == K::Hole {
                    Some(0)
                } else {
                    tensor_literal(self.source, low)
                };
                let hi = if high.kind == K::Hole {
                    shape.as_ref().and_then(|s| s.get(position)).and_then(|i| {
                        if let Index::Number(n) = i {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                } else {
                    tensor_literal(self.source, high)
                };
                // Fold literal subtraction here: the shape solver only needs a natural number.
                lo.zip(hi)
                    .and_then(|(lo, hi)| hi.checked_sub(lo))
                    .map(Index::Number)
            };
            if let (Some(output), Some(dimension)) = (&mut output, dimension) {
                output.push(dimension);
            } else {
                output = None;
            }
        }
        if sliced {
            tensor_type(output)
        } else {
            Type::named("double")
        }
    }

    fn tensor_member(&mut self, receiver: &Type, name: &str) -> Option<Type> {
        use crate::types::{is_tensor, tensor_shape, tensor_type};
        let owned = match receiver {
            Type::StoredView(inner) | Type::Borrow(inner) if is_tensor(inner) => inner.as_ref().clone(),
            _ => receiver.clone(),
        };
        match name {
            "T" => {
                let shape = tensor_shape(receiver).map(|mut shape| {
                    shape.reverse();
                    shape
                });
                Some(Type::StoredView(Box::new(tensor_type(shape))))
            }
            "copy" => Some(Type::function(vec![], owned.clone())),
            "norm" => Some(Type::function(vec![], Type::named("double"))),
            "negate" => Some(Type::function(vec![], owned.clone())),
            "power" => Some(Type::function(
                vec![Type::named("double")],
                owned.clone(),
            )),
            "divide" => Some(Type::function(
                vec![Type::named("double")],
                owned.clone(),
            )),
            "multiply" | "add" | "subtract" => Some(Type::Overloaded(
                [Type::named("double"), owned.clone()]
                    .into_iter()
                    .map(|other| {
                        let Type::Function(f) = Type::function(vec![other], owned.clone())
                        else {
                            unreachable!()
                        };
                        f
                    })
                    .collect(),
            )),
            "rightMultiply" | "rightAdd" | "rightSubtract" => Some(Type::function(
                vec![Type::named("double")],
                owned.clone(),
            )),
            "text" => Some(Type::function(vec![], Type::named("String"))),
            _ => None,
        }
    }

    fn tensor_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        use crate::types::{is_tensor, tensor_shape, tensor_type};
        let callee = ungroup(node.children.first()?);
        if callee.kind != K::Member {
            return None;
        }
        let name = self.text(&callee.children[1]).to_owned();
        let static_call = matches!(
            name.as_str(),
            "zeros" | "ones" | "filled" | "generate" | "onesLike" | "zerosLike" | "fromArray"
        );
        if !static_call
            && !matches!(
                name.as_str(),
                "checkShape"
                    | "broadcastTo"
                    | "transpose"
                    | "reshape"
                    | "map"
                    | "rank"
                    | "shape"
                    | "count"
                    | "sum"
                    | "mean"
                    | "min"
                    | "max"
                    | "negate"
                    | "power"
            )
        {
            return None;
        }
        let previous_receiver = self
            .member_receiver
            .replace(ungroup(&callee.children[0]).span);
        let receiver = self.infer(&callee.children[0], None);
        self.member_receiver = previous_receiver;
        if static_call && !matches!(&receiver, Type::Constructor(n, _) if n == "Tensor") {
            return None;
        }
        if !static_call && !is_tensor(&receiver) {
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
        let labels = match name.as_str() {
            "filled" => vec![Some("shape"), Some("value")],
            "generate" => vec![Some("shape"), Some("factory")],
            "zeros" | "ones" | "broadcastTo" | "reshape" => vec![Some("shape")],
            "map" => vec![Some("transform")],
            "rank" | "shape" | "count" => vec![],
            "sum" | "mean" | "min" | "max" if !raw.is_empty() => vec![Some("axis")],
            "onesLike" | "zerosLike" | "fromArray" => vec![Some("source")],
            "transpose" if !raw.is_empty() => vec![None],
            "power" => vec![Some("exponent")],
            _ => vec![None; raw.len()],
        };
        let Type::Function(mut skeleton) =
            Type::function(vec![Type::Invalid; labels.len()], Type::Invalid)
        else {
            unreachable!()
        };
        for (parameter, label) in skeleton.parameters.iter_mut().zip(&labels) {
            parameter.name = label.map(str::to_owned);
        }
        let Some(order) = argument_order(&skeleton, &raw) else {
            self.error(
                node,
                "E_CALL_ARGUMENTS",
                format!("Tensor.{name} has invalid arguments."),
                "Supply each declared parameter once, using its name or position.",
            );
            return Some(Type::Invalid);
        };
        let ordered = order
            .into_iter()
            .map(|i| &raw[i.unwrap()])
            .collect::<Vec<_>>();
        let inputs = ordered
            .iter()
            .map(|argument| argument.2)
            .collect::<Vec<_>>();
        let mut parameters = vec![];
        let mut arguments = vec![];
        let mut effects = false;
        let mut factory_mutates = false;
        let mut infer_arg = |checker: &mut Self, index: usize, wanted: Option<Type>| {
            let ty = checker.infer(inputs[index], wanted.as_ref());
            if let Some(wanted) = wanted {
                checker.require(inputs[index], &wanted, &ty);
                parameters.push(wanted);
            } else {
                parameters.push(ty.clone());
            }
            arguments.push((ordered[index].0.clone(), ty.clone(), inputs[index], false));
            ty
        };
        let result = match name.as_str() {
            "negate" => match &receiver {
                Type::StoredView(inner) | Type::Borrow(inner) if is_tensor(inner) => inner.as_ref().clone(),
                _ => receiver.clone(),
            },
            "power" => {
                let exponent = infer_arg(self, 0, None);
                if exponent != Type::named("Int") && exponent != Type::named("double") {
                    self.error(
                        inputs[0],
                        "E_TENSOR_OPERATOR",
                        "Tensor.power needs an Int or double exponent.",
                        "Use an integer exponent for a square matrix power, or any double exponent for an elementwise power.",
                    );
                }
                match &receiver {
                    Type::StoredView(inner) | Type::Borrow(inner) if is_tensor(inner) => inner.as_ref().clone(),
                    _ => receiver.clone(),
                }
            }
            "rank" | "count" => Type::named("Int"),
            "shape" => Type::applied("Array", vec![Type::named("Int")]),
            "map" => {
                let Type::Function(mut transform) =
                    Type::function(vec![Type::named("double")], Type::named("double"))
                else {
                    unreachable!()
                };
                transform.effects = true;
                transform.receiver_mutates = true;
                let transform = infer_arg(self, 0, Some(Type::Function(transform)));
                if let Type::Function(transform) = self.inference.resolve(&transform) {
                    effects = transform.effects;
                    factory_mutates = transform.receiver_mutates;
                }
                tensor_type(tensor_shape(&receiver))
            }
            "sum" | "mean" | "min" | "max" => {
                if inputs.is_empty() {
                    Type::named("double")
                } else {
                    infer_arg(self, 0, Some(Type::named("Int")));
                    let source = tensor_shape(&receiver);
                    let axis = tensor_signed_literal(self.source, inputs[0]);
                    match (source, axis) {
                        (Some(mut shape), Some(axis)) => {
                            if axis < 0 || axis as usize >= shape.len() {
                                self.error(
                                    inputs[0],
                                    "E_TENSOR_AXES",
                                    "Tensor axis out of range.",
                                    "Use an axis from 0 up to, but not including, the tensor rank.",
                                );
                                return Some(Type::Invalid);
                            }
                            shape.remove(axis as usize);
                            if shape.is_empty() {
                                Type::named("double")
                            } else {
                                tensor_type(Some(shape))
                            }
                        }
                        (Some(shape), _) if shape.len() == 1 => Type::named("double"),
                        _ => tensor_type(None),
                    }
                }
            }
            "onesLike" | "zerosLike" => {
                let source = infer_arg(self, 0, None);
                if !is_tensor(&source) {
                    self.error(
                        inputs[0],
                        "E_TENSOR_TYPE",
                        "onesLike and zerosLike need a tensor.",
                        "Pass a Tensor or DynTensor.",
                    );
                }
                tensor_type(tensor_shape(&source))
            }
            "fromArray" => {
                infer_arg(
                    self,
                    0,
                    Some(Type::applied("Array", vec![Type::named("double")])),
                );
                tensor_type(None)
            }
            "checkShape" => {
                let mut shape = Some(vec![]);
                for (index, input) in inputs.iter().enumerate() {
                    infer_arg(self, index, Some(Type::named("Int")));
                    if let (Some(shape), Some(n)) = (&mut shape, tensor_literal(self.source, input))
                    {
                        shape.push(Index::Number(n));
                    } else {
                        shape = None;
                    }
                }
                Type::union(tensor_type(shape), Type::named("Error"))
            }
            "transpose" => {
                let source = tensor_shape(&receiver);
                if inputs.is_empty() {
                    // Without axes every axis is reversed, as NumPy's .T does.
                    tensor_type(source.map(|mut dims| {
                        dims.reverse();
                        dims
                    }))
                } else {
                    let axes_ty = infer_arg(self, 0, None);
                    if !(matches!(&axes_ty, Type::Tuple(parts) if parts.iter().all(|t| *t == Type::named("Int")))
                        || matches!(&axes_ty, Type::Named(n, args) if n == "Array" && args == &[Type::named("Int")]))
                    {
                        self.error(
                            inputs[0],
                            "E_TENSOR_AXES",
                            "Tensor axes must be a tuple of Int values or Array[Int].",
                            "Pass each axis exactly once, counting from 0.",
                        );
                        return Some(Type::Invalid);
                    }
                    let written = ungroup(inputs[0]);
                    let axes = (written.kind == K::Tuple)
                        .then(|| {
                            written
                                .children
                                .iter()
                                .map(|n| tensor_literal(self.source, n))
                                .collect::<Option<Vec<_>>>()
                        })
                        .flatten();
                    match (source, axes) {
                        (Some(dims), Some(axes)) => {
                            if axes.len() != dims.len() {
                                self.error(
                                    inputs[0],
                                    "E_TENSOR_RANK",
                                    "transpose needs one axis for each dimension of the tensor.",
                                    "Write one axis number per dimension, such as (1, 0, 2) for three dimensions.",
                                );
                                return Some(Type::Invalid);
                            }
                            let mut seen = vec![false; dims.len()];
                            for &axis in &axes {
                                match seen.get_mut(axis as usize) {
                                    Some(slot) if !*slot => *slot = true,
                                    _ => {
                                        self.error(
                                            inputs[0],
                                            "E_TENSOR_AXES",
                                            "transpose needs each axis exactly once, counting from 0.",
                                            "Use every axis of the tensor once, such as (1, 0, 2).",
                                        );
                                        return Some(Type::Invalid);
                                    }
                                }
                            }
                            tensor_type(Some(
                                axes.iter()
                                    .map(|&axis| dims[axis as usize].clone())
                                    .collect(),
                            ))
                        }
                        _ => tensor_type(None),
                    }
                }
            }
            _ => {
                let shape_ty = infer_arg(self, 0, None);
                let rank = match &shape_ty {
                    Type::Tuple(parts) if parts.iter().all(|p| *p == Type::named("Int")) => {
                        Some(parts.len())
                    }
                    Type::Named(n, args) if n == "Array" && args == &[Type::named("Int")] => None,
                    _ => {
                        self.error(
                            inputs[0],
                            "E_TENSOR_SHAPE",
                            "A tensor shape must be a tuple of Int dimensions or Array[Int].",
                            "Pass a shape tuple, an Array[Int], or another tensor's shape().",
                        );
                        return Some(Type::Invalid);
                    }
                };
                let raw = ungroup(inputs[0]);
                let shape = (raw.kind == K::Tuple)
                    .then(|| {
                        raw.children
                            .iter()
                            .map(|n| tensor_literal(self.source, n).map(Index::Number))
                            .collect::<Option<Vec<_>>>()
                    })
                    .flatten();
                if name == "filled" {
                    infer_arg(self, 1, Some(Type::named("double")));
                }
                if name == "generate" {
                    let rank = rank
                        .or_else(|| {
                            ungroup(inputs[1])
                                .child(K::LambdaParameters)
                                .map(|p| p.children.len())
                        })
                        .or_else(|| {
                            if let Type::Function(f) = self.infer(inputs[1], None) {
                                Some(f.parameters.len())
                            } else {
                                None
                            }
                        });
                    let Some(rank) = rank else {
                        self.error(inputs[1], "E_TENSOR_FACTORY", "Tensor.generate needs a function with a known parameter count.", "Write one Int parameter for each dimension; an Array shape's rank is checked at run time.");
                        return Some(Type::Invalid);
                    };
                    let Type::Function(mut factory) =
                        Type::function(vec![Type::named("Int"); rank], Type::named("double"))
                    else {
                        unreachable!()
                    };
                    factory.effects = true;
                    factory.receiver_mutates = true;
                    let factory = infer_arg(self, 1, Some(Type::Function(factory)));
                    if let Type::Function(factory) = self.inference.resolve(&factory) {
                        effects = factory.effects;
                        factory_mutates = factory.receiver_mutates;
                    }
                }
                if name == "reshape"
                    && let (Some(source), Some(target)) = (tensor_shape(&receiver), shape.as_ref())
                    && tensor_element_counts_match(&source, target) == Some(false)
                {
                    self.error(
                        node,
                        "E_TENSOR_SHAPE",
                        "Tensor reshape needs the same number of elements.",
                        "Choose dimensions whose product equals the source element count.",
                    );
                }
                if name == "broadcastTo"
                    && let (Some(source), Some(target)) = (tensor_shape(&receiver), shape.as_ref())
                {
                    let compatible = source.len() <= target.len()
                        && source.iter().rev().zip(target.iter().rev()).all(|(a, b)| {
                            *a == Index::Number(1)
                                || self.inference.clone().unify(
                                    &Type::Indexed(Box::new(Type::named("Nat")), a.clone()),
                                    &Type::Indexed(Box::new(Type::named("Nat")), b.clone()),
                                )
                        });
                    if !compatible {
                        self.error(node, "E_TENSOR_BROADCAST", "The static source shape cannot broadcast to this target shape.", "Align trailing dimensions; each source dimension must equal its target or be 1.");
                    }
                }
                tensor_type(shape)
            }
        };
        let Type::Function(mut function) = Type::function(parameters, result) else {
            unreachable!()
        };
        for (parameter, label) in function.parameters.iter_mut().zip(labels) {
            parameter.name = label.map(str::to_owned);
        }
        arguments.sort_by_key(|argument| argument.2.span.start);
        function.callable = Some(Callable::Builtin(format!("std.tensor.{name}")));
        function.receiver = (!static_call).then(|| Box::new(receiver));
        function.effects = effects;
        if factory_mutates {
            function.parameters[usize::from(name == "generate")].passing = Passing::Mut;
        }
        let result = self.apply(node, Type::Function(function), &arguments, expected);
        Some(result)
    }
}

fn tensor_indices(index: &Node) -> Vec<&Node> {
    if index.kind == K::Tuple {
        index.children.iter().collect()
    } else {
        vec![index]
    }
}

fn tensor_literal(source: &SourceFile, node: &Node) -> Option<u64> {
    let node = ungroup(node);
    if node.kind != K::Literal {
        return None;
    }
    let text = source.slice(node.span).replace('_', "");
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        text.parse().ok()
    }
}

fn tensor_signed_literal(source: &SourceFile, node: &Node) -> Option<i64> {
    let node = ungroup(node);
    if node.kind == K::Unary && source.slice(node.children[0].span) == "-" {
        let n = tensor_literal(source, &node.children[1])?;
        i64::try_from(-(i128::from(n))).ok()
    } else {
        i64::try_from(tensor_literal(source, node)?).ok()
    }
}

// Cancel integer factors instead of overflowing while comparing literal shape products.
fn tensor_element_counts_match(left: &[Index], right: &[Index]) -> Option<bool> {
    let numbers = |shape: &[Index]| {
        shape
            .iter()
            .map(|d| {
                if let Index::Number(n) = d {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect::<Option<Vec<_>>>()
    };
    let (mut left, mut right) = (numbers(left)?, numbers(right)?);
    if left.contains(&0) || right.contains(&0) {
        return Some(left.contains(&0) == right.contains(&0));
    }
    for a in &mut left {
        for b in &mut right {
            let (mut x, mut y) = (*a, *b);
            while y != 0 {
                (x, y) = (y, x % y);
            }
            *a /= x;
            *b /= x;
        }
    }
    Some(left.iter().chain(&right).all(|n| *n == 1))
}
