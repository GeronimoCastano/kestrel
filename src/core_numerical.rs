impl Builder<'_, '_> {
    fn probability_call_value(
        &mut self,
        target: &str,
        values: Vec<Value>,
        signature: &crate::types::FunctionType,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let Some(rest) = target.strip_prefix("std.prob.") else {
            return self.unsupported(node, "this probability target");
        };
        let Some((class, method)) = rest.split_once('.') else {
            return self.unsupported(node, "this probability method");
        };
        let Some(distribution) = crate::probability::find(class) else {
            return self.unsupported(node, "this probability distribution");
        };
        let receiver = signature.receiver.is_some();
        let mut atoms = vec![];
        let operation;
        if method == "create" {
            atoms.extend(values.iter().map(|value| value.atom.clone()));
            operation = format!("prob.{class}.create");
        } else if matches!(method, "pdf" | "pmf" | "logPdf" | "logPmf" | "cdf") {
            let mode = if method == "cdf" {
                "cdf"
            } else if method.starts_with("log") {
                "log"
            } else {
                "density"
            };
            if receiver {
                atoms.push(values[0].atom.clone());
                atoms.push(values[1].atom.clone());
                operation = format!("prob.{class}.{mode}Instance");
            } else {
                atoms.extend(values.iter().map(|value| value.atom.clone()));
                operation = format!("prob.{class}.{mode}Static");
            }
        } else if method == "sample" {
            if receiver {
                atoms.push(values[0].atom.clone());
                atoms.push(values[1].atom.clone());
                operation = format!("prob.{class}.sampleInstance");
            } else {
                let position = |name: &str| {
                    signature
                        .parameters
                        .iter()
                        .position(|parameter| parameter.name.as_deref() == Some(name))
                        .map(|index| values[index].atom.clone())
                };
                for (name, _) in distribution.parameters {
                    let Some(atom) = position(name) else {
                        return self.unsupported(node, "this probability sample");
                    };
                    atoms.push(atom);
                }
                let rng = position("rng");
                if let Some(rng) = &rng {
                    atoms.push(rng.clone());
                }
                let shape = position("shape");
                if let Some(shape) = &shape {
                    atoms.push(shape.clone());
                }
                operation = format!(
                    "prob.{class}.sampleStatic{}{}",
                    if rng.is_some() { "Rng" } else { "" },
                    if shape.is_some() { "Tensor" } else { "" }
                );
            }
        } else {
            return self.unsupported(node, "this probability method");
        }
        Ok(self.temporary(
            result,
            Rhs::Builtin(format!("numeric.{operation}"), Ty::Unit, atoms),
            node,
        ))
    }
}

impl Builder<'_, '_> {
    fn numerical_tensor_input(&mut self, value: Value, node: &Node) -> Result<Value> {
        let ty = self.mono.apply(&value.ty);
        if !crate::types::is_tensor(&ty) || !matches!(value.native, Ty::Ref(_)) {
            return Ok(value);
        }
        let native = self.native_type(&ty, node)?;
        let mut copy = self.temporary(
            native,
            Rhs::Builtin("tensor.contiguous".into(), native, vec![value.atom]),
            node,
        );
        copy.ty = ty;
        copy.keepalive = value.keepalive;
        Ok(copy)
    }

    fn numerical_call_value(
        &mut self,
        target: &str,
        values: Vec<Value>,
        signature: &crate::types::FunctionType,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let mut values = values
            .into_iter()
            .map(|value| self.numerical_tensor_input(value, node))
            .collect::<Result<Vec<_>>>()?;
        let mut atoms = values.drain(..).map(|v| v.atom).collect::<Vec<_>>();
        let mut operation = target.strip_prefix("std.").unwrap().to_owned();
        if target == "std.random.seed" {
            return Ok(self.temporary(
                result,
                Rhs::Builtin("numeric.random.seed".into(), Ty::Unit, atoms),
                node,
            ));
        }
        if target.starts_with("std.stats.") {
            let mut axis = Atom::Int(0);
            let mut ddof = Atom::Int(0);
            let mut total = true;
            for (p, atom) in signature.parameters.iter().zip(&atoms) {
                match p.name.as_deref() {
                    Some("axis") => {
                        axis = atom.clone();
                        total = false;
                    }
                    Some("ddof") => ddof = atom.clone(),
                    _ => {}
                }
            }
            atoms.truncate(1);
            atoms.extend([axis, ddof, Atom::Bool(total)]);
        }
        if matches!(
            target,
            "std.linalg.pinv" | "std.linalg.rank" | "std.linalg.leastSquares"
        ) {
            let has_tolerance = signature
                .parameters
                .last()
                .is_some_and(|p| p.name.as_deref() == Some("rcond"));
            if !has_tolerance {
                atoms.push(Atom::Double(0.0));
            }
            atoms.push(Atom::Bool(!has_tolerance));
        }
        if target == "std.linalg.LU.solve" {
            operation = "linalg.factorSolve".into();
        }
        Ok(self.temporary(
            result,
            Rhs::Builtin(format!("numeric.{operation}"), Ty::Unit, atoms),
            node,
        ))
    }
}
