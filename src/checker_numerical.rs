impl Checker<'_> {
    /// `Normal.sample(...)` and friends: the no-object probability API. The
    /// distribution parameters are ordinary named arguments, `shape` is the
    /// optional sample shape, and an explicit `rng` removes the hidden default
    /// generator and its Effects requirement.
    fn probability_static_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        use crate::types::{tensor_type, Type};
        let callee = ungroup(node.children.first()?);
        if callee.kind != K::Member {
            return None;
        }
        let receiver = ungroup(callee.children.first()?);
        let spelling = self.text(receiver);
        let last = spelling.rsplit('.').next().unwrap_or_default();
        let _ = crate::probability::find(last)?;
        let previous = self.member_receiver.replace(receiver.span);
        let owner = self.infer(receiver, None);
        self.member_receiver = previous;
        let Type::Constructor(id, _) = self.inference.resolve(&owner) else {
            return None;
        };
        let distribution = crate::probability::DISTRIBUTIONS
            .iter()
            .find(|d| d.class_path() == id)?;
        let operation = self.text(&callee.children[1]);
        if operation != "sample" {
            return None;
        }

        let raw = node.children.iter().skip(1).collect::<Vec<_>>();
        let first_positional_rng = raw.first().is_some_and(|first| {
            first.kind != K::NamedArgument
                && first.children.last().is_some_and(|input| {
                    let ty = self.infer(input, None);
                    matches!(
                        self.inference.resolve(&ty),
                        Type::Named(name, _) if name == "std.random.Rng"
                    )
                })
        });
        let named_rng = raw.iter().any(|argument| {
            argument.kind == K::NamedArgument && self.text(&argument.children[0]) == "rng"
        });
        let has_rng = first_positional_rng || named_rng;

        let parameter_names = distribution
            .parameters
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        let mut allowed = parameter_names.clone();
        allowed.push("shape");
        if has_rng {
            allowed.push("rng");
        }
        let positional_labels = if first_positional_rng {
            let mut labels = vec!["rng"];
            labels.extend(parameter_names.iter().copied());
            labels.push("shape");
            labels
        } else {
            let mut labels = parameter_names.clone();
            labels.push("shape");
            labels
        };
        let mut supplied = BTreeMap::new();
        let mut positional = 0;
        for argument in &raw {
            let (label, input) = if argument.kind == K::NamedArgument {
                (
                    self.text(&argument.children[0]).to_owned(),
                    &argument.children[1],
                )
            } else {
                let label = positional_labels
                    .get(positional)
                    .copied()
                    .unwrap_or("")
                    .to_owned();
                positional += 1;
                (label, argument.children.last().unwrap())
            };
            if !allowed.contains(&label.as_str()) || supplied.insert(label, input).is_some() {
                self.error(
                    argument,
                    "E_CALL_ARGUMENTS",
                    "Invalid or repeated probability argument.",
                    "Use the named parameters shown by signature help.",
                );
                return Some(Type::Invalid);
            }
        }
        for label in distribution.parameters.iter().map(|(name, _)| *name) {
            if !supplied.contains_key(label) {
                self.error(
                    node,
                    "E_CALL_ARGUMENTS",
                    format!("Missing `{label}` for {}.", distribution.name),
                    "Supply each distribution parameter.",
                );
                return Some(Type::Invalid);
            }
        }

        let mut labels = vec![];
        if has_rng {
            labels.push("rng");
        }
        labels.extend(parameter_names.iter().copied());
        labels.push("shape");
        let mut arguments = vec![];
        let mut parameters = vec![];
        let mut used_labels = vec![];
        for label in labels {
            let Some(input) = supplied.get(label).copied() else {
                continue;
            };
            let hint = if label == "rng" {
                Some(Type::named("std.random.Rng"))
            } else if let Some((_, kind)) = distribution
                .parameters
                .iter()
                .find(|(name, _)| *name == label)
            {
                Some(match kind {
                    crate::probability::Parameter::Double => Type::named("double"),
                    crate::probability::Parameter::Int => Type::named("Int"),
                })
            } else {
                None
            };
            let ty = self.infer(input, hint.as_ref());
            let ty = self.inference.resolve(&ty);
            if let Some(hint) = hint {
                self.require(input, &hint, &ty);
            }
            arguments.push((Some(label.into()), ty.clone(), input, false));
            parameters.push(ty);
            used_labels.push(label);
        }

        let result = if let Some(shape) = supplied.get("shape").copied() {
            let shape_type = self.infer(shape, None);
            let shape_type = self.inference.resolve(&shape_type);
            let static_shape = match &shape_type {
                Type::Named(name, args) if name == "Int" && args.is_empty() => {
                    tensor_signed_literal(self.source, shape).map(|n| {
                        if n < 0 {
                            self.error(
                                shape,
                                "E_PROB_SHAPE",
                                "A probability sample shape cannot be negative.",
                                "Use zero or a positive length.",
                            );
                            Type::Invalid
                        } else {
                            tensor_type(Some(vec![Index::Number(n as u64)]))
                        }
                    })
                }
                Type::Named(name, args)
                    if matches!(name.as_str(), "Array" | "List")
                        && args == &[Type::named("Int")] =>
                {
                    Some(tensor_type(None))
                }
                Type::Tuple(parts)
                    if parts.iter().all(|part| *part == Type::named("Int")) =>
                {
                    let written = ungroup(shape);
                    let dimensions = if written.kind == K::Tuple {
                        written
                            .children
                            .iter()
                            .map(|part| tensor_signed_literal(self.source, part))
                            .collect::<Option<Vec<_>>>()
                    } else {
                        None
                    };
                    Some(match dimensions {
                        Some(dimensions) if dimensions.iter().all(|n| *n >= 0) => tensor_type(Some(
                            dimensions
                                .into_iter()
                                .map(|n| Index::Number(n as u64))
                                .collect(),
                        )),
                        Some(_) => {
                            self.error(
                                shape,
                                "E_PROB_SHAPE",
                                "A probability sample shape cannot be negative.",
                                "Use zero or positive dimensions.",
                            );
                            Type::Invalid
                        }
                        None => tensor_type(None),
                    })
                }
                _ => {
                    self.error(
                        shape,
                        "E_PROB_SHAPE",
                        "A probability sample shape must be an Int, a tuple of Int, or an Array/List of Int.",
                        "Pass a length, a shape tuple, or an integer collection.",
                    );
                    Some(Type::Invalid)
                }
            };
            static_shape.unwrap_or_else(|| tensor_type(None))
        } else {
            distribution.sample_result()
        };

        let Type::Function(mut function) = Type::function(parameters, result) else {
            unreachable!()
        };
        for (parameter, label) in function.parameters.iter_mut().zip(used_labels) {
            parameter.name = Some(label.into());
        }
        if let Some(first) = function.parameters.first_mut()
            && has_rng
        {
            first.passing = crate::types::Passing::Mut;
        }
        function.effects = !has_rng;
        function.callable = Some(Callable::Builtin(format!(
            "std.prob.{}.sample",
            distribution.name
        )));
        arguments.sort_by_key(|argument| argument.2.span.start);
        Some(self.apply(node, Type::Function(function), &arguments, expected))
    }

    fn numerical_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        use crate::types::{is_tensor, tensor_shape, tensor_type};
        let callee = ungroup(node.children.first()?);
        let spelling = match callee.kind {
            K::Name => self.text(callee),
            K::Member => self.text(&callee.children[1]),
            _ => return None,
        };
        if !crate::numerical::UNARY.contains(&spelling)
            && !crate::numerical::LINALG.contains(&spelling)
            && !crate::numerical::STATS.contains(&spelling)
        {
            return None;
        }
        if callee.kind == K::Member {
            let previous_receiver = self
                .member_receiver
                .replace(ungroup(&callee.children[0]).span);
            let owner = self.infer(&callee.children[0], None);
            self.member_receiver = previous_receiver;
            if !matches!(&owner, Type::Namespace(_))
                && !matches!(&owner, Type::Named(n, _) if n == "std.linalg.LU")
            {
                return None;
            }
        }
        let previous = self.direct_callee.replace(callee.span);
        let ty = self.infer(callee, None);
        self.direct_callee = previous;
        let f = match &ty {
            Type::Function(f) => f,
            Type::Overloaded(fs) => fs.first()?,
            _ => return None,
        };
        let Some(Callable::Builtin(target)) = &f.callable else {
            return None;
        };
        let target = target.clone();
        let math = target.starts_with("std.math.")
            && crate::numerical::UNARY.contains(&target.rsplit('.').next()?);
        let stats = target.starts_with("std.stats.");
        let lu_solve = target == "std.linalg.LU.solve";
        if !math && !stats && !target.starts_with("std.linalg.") {
            return None;
        }
        let operation = target.rsplit('.').next().unwrap();
        let labels = if math {
            vec!["value"]
        } else if stats {
            if matches!(operation, "variance" | "stddev") {
                vec!["source", "axis", "ddof"]
            } else if matches!(operation, "covariance" | "correlation") {
                vec!["source", "ddof"]
            } else {
                vec!["source", "axis"]
            }
        } else if lu_solve {
            vec!["b"]
        } else if matches!(operation, "pinv" | "rank") {
            vec!["a", "rcond"]
        } else if operation == "leastSquares" {
            vec!["a", "b", "rcond"]
        } else if matches!(
            operation,
            "dot" | "outer" | "solve" | "solveLower" | "solveUpper"
        ) {
            vec!["a", "b"]
        } else {
            vec!["a"]
        };
        let mut supplied = BTreeMap::new();
        let mut positional = 0;
        for argument in node.children.iter().skip(1) {
            let (label, input) = if argument.kind == K::NamedArgument {
                (
                    self.text(&argument.children[0]).to_owned(),
                    &argument.children[1],
                )
            } else {
                let label = labels.get(positional).copied().unwrap_or("").to_owned();
                positional += 1;
                (label, argument.children.last().unwrap())
            };
            if !labels.contains(&label.as_str()) || supplied.insert(label, input).is_some() {
                self.error(
                    argument,
                    "E_CALL_ARGUMENTS",
                    "Invalid or repeated numerical argument.",
                    "Use the named parameters shown by signature help.",
                );
                return Some(Type::Invalid);
            }
        }
        let mandatory = if lu_solve || stats || math {
            1
        } else if matches!(
            operation,
            "dot" | "outer" | "solve" | "solveLower" | "solveUpper" | "leastSquares"
        ) {
            2
        } else {
            1
        };
        if labels
            .iter()
            .take(mandatory)
            .any(|label| !supplied.contains_key(*label))
        {
            self.error(
                node,
                "E_CALL_ARGUMENTS",
                "Missing numerical argument.",
                "Supply each required tensor or scalar.",
            );
            return Some(Type::Invalid);
        }
        let mut arguments = vec![];
        let mut parameters = vec![];
        let mut used_labels = vec![];
        for label in labels {
            let Some(input) = supplied.get(label).copied() else {
                continue;
            };
            let hint = if label == "axis" || label == "ddof" {
                Some(Type::named("Int"))
            } else if label == "rcond" {
                Some(Type::named("double"))
            } else {
                None
            };
            let ty = self.infer(input, hint.as_ref());
            let ty = self.inference.resolve(&ty);
            if let Some(hint) = hint {
                self.require(input, &hint, &ty);
            }
            arguments.push((Some(label.into()), ty.clone(), input, false));
            parameters.push(ty);
            used_labels.push(label);
        }
        let first = parameters.first()?.clone();
        if !math && !is_tensor(&first) {
            self.error(
                node,
                "E_LINALG_TYPE",
                "This numerical operation needs a tensor of doubles.",
                "Construct a Tensor or DynTensor first.",
            );
            return Some(Type::Invalid);
        }
        if math && !is_tensor(&first) {
            return None;
        }
        let a = tensor_shape(&first);
        let b = parameters.get(1).and_then(tensor_shape);
        let a_rank = if math
            || stats && !matches!(operation, "covariance" | "correlation")
            || matches!(operation, "norm" | "frobeniusNorm")
        {
            0
        } else if matches!(operation, "dot" | "outer") {
            1
        } else if lu_solve {
            0
        } else {
            2
        };
        if a_rank != 0 && a.as_ref().is_some_and(|s| s.len() != a_rank) {
            self.error(
                node,
                "E_LINALG_RANK",
                format!("{operation} needs rank {a_rank}."),
                "Pass vectors for dot/outer and matrices for factorizations and solves.",
            );
            return Some(Type::Invalid);
        }
        let two_tensors = !math && !stats && !lu_solve && mandatory == 2;
        if two_tensors {
            if !parameters.get(1).is_some_and(is_tensor) {
                self.error(
                    node,
                    "E_LINALG_TYPE",
                    "Both numerical operands must be tensors.",
                    "Pass a vector or matrix of doubles.",
                );
                return Some(Type::Invalid);
            }
            if b.as_ref().is_some_and(|s| {
                if matches!(operation, "dot" | "outer") {
                    s.len() != 1
                } else {
                    !matches!(s.len(), 1 | 2)
                }
            }) {
                self.error(
                    node,
                    "E_LINALG_RANK",
                    "The right operand has the wrong rank.",
                    "Use a vector for dot/outer, or a vector/matrix for a linear solve.",
                );
                return Some(Type::Invalid);
            }
        }
        let mut match_dimension = |left: &Index, right: &Index| {
            let dim = |index: &Index| Type::Indexed(Box::new(Type::named("Nat")), index.clone());
            if !self.inference.unify(&dim(left), &dim(right)) {
                self.error(
                    node,
                    "E_LINALG_SHAPE",
                    "Numerical dimensions do not match.",
                    "Match matrix rows, vector lengths and square-matrix dimensions.",
                );
            }
        };
        if !math
            && !stats
            && !lu_solve
            && !matches!(
                operation,
                "dot"
                    | "outer"
                    | "norm"
                    | "frobeniusNorm"
                    | "qr"
                    | "svd"
                    | "pinv"
                    | "rank"
                    | "spectralNorm"
                    | "leastSquares"
                    | "lu"
            )
            && let Some(a) = &a
        {
            match_dimension(&a[0], &a[1]);
        }
        if (operation == "dot" || two_tensors && !matches!(operation, "outer"))
            && let (Some(a), Some(b)) = (&a, &b)
        {
            match_dimension(&a[0], &b[0]);
        }
        if lu_solve {
            if let Some(Type::Named(_, fields)) = f.receiver.as_deref()
                && let Some(shape) = fields.first().and_then(tensor_shape)
            {
                match_dimension(&shape[0], &shape[1]);
                if let Some(b) = &a
                    && !b.is_empty()
                {
                    match_dimension(&shape[0], &b[0]);
                }
            }
            if a.as_ref().is_some_and(|s| !matches!(s.len(), 1 | 2)) {
                self.error(
                    node,
                    "E_LINALG_RANK",
                    "A solve needs a vector or matrix right-hand side.",
                    "Use rank 1 or 2.",
                );
                return Some(Type::Invalid);
            }
        }
        let static_tensor = |shape: Option<Vec<Index>>| tensor_type(shape);
        let min_dim = a.as_ref().and_then(|s| {
            if s.len() != 2 {
                None
            } else {
                match (&s[0], &s[1]) {
                    (Index::Number(m), Index::Number(n)) => Some(Index::Number((*m).min(*n))),
                    (m, n) if m == n => Some(m.clone()),
                    _ => None,
                }
            }
        });
        let class =
            |name: &str, fields: Vec<Type>| Type::applied(&format!("std.linalg.{name}"), fields);
        let result = if math {
            first.clone()
        } else if stats {
            if matches!(operation, "covariance" | "correlation") {
                static_tensor(a.as_ref().map(|s| vec![s[1].clone(), s[1].clone()]))
            } else if let Some(axis) = supplied.get("axis") {
                match (a.clone(), tensor_signed_literal(self.source, axis)) {
                    (Some(mut shape), Some(axis)) if axis >= 0 && (axis as usize) < shape.len() => {
                        shape.remove(axis as usize);
                        if shape.is_empty() {
                            Type::named("double")
                        } else {
                            static_tensor(Some(shape))
                        }
                    }
                    (Some(_), Some(_)) => {
                        self.error(
                            axis,
                            "E_TENSOR_AXES",
                            "Tensor axis out of range.",
                            "Use an existing tensor axis.",
                        );
                        Type::Invalid
                    }
                    (Some(shape), _) if shape.len() == 1 => Type::named("double"),
                    _ => static_tensor(None),
                }
            } else {
                Type::named("double")
            }
        } else {
            match operation {
                "dot" | "norm" | "frobeniusNorm" | "spectralNorm" | "det" => Type::named("double"),
                "rank" => Type::named("Int"),
                "outer" => static_tensor(
                    a.as_ref()
                        .zip(b.as_ref())
                        .map(|(a, b)| vec![a[0].clone(), b[0].clone()]),
                ),
                "solve" if lu_solve => first.clone(),
                "solve" | "solveLower" | "solveUpper" => parameters[1].clone(),
                "leastSquares" => static_tensor(a.as_ref().zip(b.as_ref()).map(|(a, b)| {
                    let mut s = vec![a[1].clone()];
                    s.extend_from_slice(&b[1..]);
                    s
                })),
                "pinv" => static_tensor(a.as_ref().map(|a| vec![a[1].clone(), a[0].clone()])),
                "lu" => class(
                    "LU",
                    vec![
                        first.clone(),
                        Type::applied("Array", vec![Type::named("Int")]),
                    ],
                ),
                "qr" => class(
                    "QR",
                    vec![
                        static_tensor(
                            a.as_ref()
                                .zip(min_dim.as_ref())
                                .map(|(a, k)| vec![a[0].clone(), k.clone()]),
                        ),
                        static_tensor(
                            a.as_ref()
                                .zip(min_dim.as_ref())
                                .map(|(a, k)| vec![k.clone(), a[1].clone()]),
                        ),
                    ],
                ),
                "svd" => class(
                    "SVD",
                    vec![
                        static_tensor(
                            a.as_ref()
                                .zip(min_dim.as_ref())
                                .map(|(a, k)| vec![a[0].clone(), k.clone()]),
                        ),
                        static_tensor(min_dim.as_ref().map(|k| vec![k.clone()])),
                        static_tensor(
                            a.as_ref()
                                .zip(min_dim.as_ref())
                                .map(|(a, k)| vec![k.clone(), a[1].clone()]),
                        ),
                    ],
                ),
                "eigh" => class(
                    "Eigh",
                    vec![
                        static_tensor(a.as_ref().map(|a| vec![a[0].clone()])),
                        first.clone(),
                    ],
                ),
                _ => first.clone(),
            }
        };
        let Type::Function(mut function) = Type::function(parameters, result) else {
            unreachable!()
        };
        for (parameter, label) in function.parameters.iter_mut().zip(used_labels) {
            parameter.name = Some(label.into());
        }
        function.callable = Some(Callable::Builtin(target));
        function.receiver = f.receiver.clone();
        arguments.sort_by_key(|a| a.2.span.start);
        Some(self.apply(node, Type::Function(function), &arguments, expected))
    }
}
