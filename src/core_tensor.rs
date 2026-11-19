impl Builder<'_, '_> {
    fn tensor_dimensions(&mut self, value: Value, node: &Node) -> Result<Vec<Atom>> {
        let (Ty::Record(id) | Ty::Ref(id)) = value.native else {
            return self.unsupported(node, "this tensor shape");
        };
        Ok(self.compiler.layouts[id]
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| Atom::Field(Box::new(value.atom.clone()), i, f.ty))
            .collect())
    }

    fn tensor_shape_result(
        &mut self,
        operation: &str,
        mut leading: Vec<Atom>,
        shape: Value,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let array = matches!(shape.native, Ty::Record(id) | Ty::Ref(id) if matches!(self.compiler.layouts[id].kind, LayoutKind::Array(Ty::Int)));
        let operation = if array {
            leading.push(shape.atom);
            format!("tensor.{operation}ArrayShape")
        } else {
            leading.extend(self.tensor_dimensions(shape, node)?);
            format!("tensor.{operation}")
        };
        Ok(self.temporary(result, Rhs::Builtin(operation, Ty::Unit, leading), node))
    }

    fn tensor_index_atoms(&mut self, node: &Node) -> Result<(Value, Vec<Atom>, bool)> {
        let owner = self.expression(&node.children[0])?;
        let index = &node.children[1];
        let parts = if index.kind == K::Tuple {
            index.children.iter().collect::<Vec<_>>()
        } else {
            vec![index]
        };
        let sliced = parts.iter().any(|p| p.kind == K::Slice);
        let mut atoms = vec![owner.atom.clone()];
        for part in parts {
            if part.kind == K::Slice {
                let mut flags = 1;
                let mut bounds = vec![];
                for (i, bound) in part.children.iter().enumerate() {
                    if bound.kind == K::Hole {
                        flags |= 2 << i;
                        bounds.push(Atom::Int(0));
                    } else {
                        let value = self.expression(bound)?;
                        let value = self.freeze(value, bound)?;
                        bounds.push(value.atom);
                    }
                }
                atoms.push(Atom::Int(flags));
                atoms.extend(bounds);
            } else {
                let value = self.expression(part)?;
                let value = self.freeze(value, part)?;
                if sliced {
                    atoms.extend([Atom::Int(0), value.atom, Atom::Int(0)]);
                } else {
                    atoms.push(value.atom);
                }
            }
        }
        Ok((owner, atoms, sliced))
    }

    fn tensor_index_value(&mut self, node: &Node) -> Result<Value> {
        let (_, atoms, sliced) = self.tensor_index_atoms(node)?;
        let result = self.native_type(&self.ty(node), node)?;
        Ok(self.temporary(
            result,
            Rhs::Builtin(
                if sliced { "tensor.slice" } else { "tensor.at" }.into(),
                Ty::Unit,
                atoms,
            ),
            node,
        ))
    }

    fn tensor_store(&mut self, target: &Node, value: Value, node: &Node) -> Result<()> {
        let (_, mut atoms, _) = self.tensor_index_atoms(target)?;
        atoms.insert(1, value.atom);
        self.code.push(Statement::Evaluate(
            Rhs::Builtin("tensor.set".into(), Ty::Unit, atoms),
            self.location(node),
        ));
        Ok(())
    }

    fn tensor_assign(&mut self, node: &Node) -> Result<Value> {
        let (_, mut atoms, _) = self.tensor_index_atoms(&node.children[0])?;
        let value = self.expression(&node.children[1])?;
        atoms.insert(1, value.atom);
        Ok(self.temporary(
            Ty::Unit,
            Rhs::Builtin("tensor.set".into(), Ty::Unit, atoms),
            node,
        ))
    }

    fn tensor_call_value(
        &mut self,
        method: &str,
        mut values: Vec<Value>,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        if method == "construct" {
            let source = values.remove(0);
            let (Ty::Record(id) | Ty::Ref(id)) = source.native else {
                return self.unsupported(node, "this tensor input");
            };
            let array = matches!(self.compiler.layouts[id].kind, LayoutKind::Array(_));
            let length = self.temporary(
                Ty::Int,
                Rhs::Copy(if array {
                    Atom::ArrayLength(Box::new(source.atom.clone()))
                } else {
                    Atom::CollectionLength(Box::new(source.atom.clone()))
                }),
                node,
            );
            let output = if let Some(shape) = values.pop() {
                self.tensor_shape_result(
                    "construct",
                    vec![length.atom.clone()],
                    shape,
                    result,
                    node,
                )?
            } else {
                self.temporary(
                    result,
                    Rhs::Builtin(
                        "tensor.construct".into(),
                        Ty::Unit,
                        vec![length.atom.clone(), length.atom.clone()],
                    ),
                    node,
                )
            };
            self.tensor_loop(length.atom, node, |builder, index| {
                let item = if array {
                    Atom::ArrayItem(Box::new(source.atom), Box::new(index.clone()), Ty::Double)
                } else {
                    Atom::ListItem(Box::new(source.atom), Box::new(index.clone()), Ty::Double)
                };
                builder.code.push(Statement::Evaluate(
                    Rhs::Builtin(
                        "tensor.setFlat".into(),
                        Ty::Unit,
                        vec![output.atom.clone(), index, item],
                    ),
                    builder.location(node),
                ));
                Ok(())
            })?;
            return Ok(output);
        }
        let op = match method {
            "multiply" => {
                if values[1].native == Ty::Double {
                    "scale"
                } else {
                    "multiply"
                }
            }
            "rightMultiply" => "scale",
            "add" if values[1].native == Ty::Double => "add_scalar",
            "rightAdd" => "add_scalar",
            "subtract" if values[1].native == Ty::Double => "subtract_scalar",
            "rightSubtract" => "right_subtract",
            "negate" => "negate",
            "power" => {
                if values[1].native == Ty::Int {
                    "powerInt"
                } else {
                    "powerDouble"
                }
            }
            "count" => "length",
            "broadcastTo" => "broadcast",
            other => other,
        };
        if matches!(method, "zeros" | "ones" | "filled" | "generate") {
            let shape = values.remove(0);
            let fill = match method {
                "filled" => values.remove(0).atom,
                "ones" => Atom::Double(1.0),
                _ => Atom::Double(0.0),
            };
            let output = self.tensor_shape_result("new", vec![fill], shape, result, node)?;
            if method != "generate" {
                return Ok(output);
            }
            let factory = values.remove(0);
            let Type::Function(signature) = &factory.ty else {
                return self.unsupported(node, "a generator without a function type");
            };
            let rank = signature.parameters.len();
            self.code.push(Statement::Evaluate(
                Rhs::Builtin(
                    "tensor.requireRank".into(),
                    Ty::Unit,
                    vec![output.atom.clone(), Atom::Int(rank as i64)],
                ),
                self.location(node),
            ));
            let length = self.temporary(
                Ty::Int,
                Rhs::Builtin("tensor.length".into(), Ty::Unit, vec![output.atom.clone()]),
                node,
            );
            let index = self.local(Ty::Int, false, false);
            self.code.push(Statement::Set(
                index,
                Rhs::Copy(Atom::Int(0)),
                self.location(node),
            ));
            let (condition, condition_code) = self.isolated(|builder| {
                Ok(builder.temporary(
                    Ty::Bool,
                    Rhs::Binary("<".into(), Ty::Int, Atom::Slot(index), length.atom),
                    node,
                ))
            })?;
            let (_, body) = self.isolated(|builder| {
                let mut coordinates = vec![];
                for d in 0..rank {
                    coordinates.push(builder.temporary(
                        Ty::Int,
                        Rhs::Builtin(
                            "tensor.coordinate".into(),
                            Ty::Unit,
                            vec![output.atom.clone(), Atom::Slot(index), Atom::Int(d as i64)],
                        ),
                        node,
                    ));
                }
                let item = builder.callback(&factory, coordinates, node)?;
                builder.code.push(Statement::Evaluate(
                    Rhs::Builtin(
                        "tensor.setFlat".into(),
                        Ty::Unit,
                        vec![output.atom.clone(), Atom::Slot(index), item.atom],
                    ),
                    builder.location(node),
                ));
                Ok(Value::unit())
            })?;
            let step = vec![Statement::Set(
                index,
                Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
                self.location(node),
            )];
            let id = self.next_loop;
            self.next_loop += 1;
            self.code.push(Statement::Loop {
                id,
                condition: condition_code,
                value: condition.atom,
                body,
                step,
            });
            return Ok(output);
        }
        if matches!(method, "broadcastTo" | "reshape" | "transpose") && values.len() == 2 {
            let shape = values.pop().unwrap();
            let owner = values.remove(0);
            let operation = if method == "broadcastTo" {
                "broadcast"
            } else if method == "transpose" {
                "transposeAxes"
            } else {
                method
            };
            return self.tensor_shape_result(operation, vec![owner.atom], shape, result, node);
        }
        if method == "map" {
            let source = values.remove(0);
            let transform = values.remove(0);
            let output = self.temporary(
                result,
                Rhs::Builtin("tensor.copy".into(), Ty::Unit, vec![source.atom.clone()]),
                node,
            );
            let length = self.temporary(
                Ty::Int,
                Rhs::Builtin("tensor.length".into(), Ty::Unit, vec![output.atom.clone()]),
                node,
            );
            self.tensor_loop(length.atom, node, |builder, index| {
                let item = builder.temporary(
                    Ty::Double,
                    Rhs::Builtin(
                        "tensor.getFlat".into(),
                        Ty::Unit,
                        vec![output.atom.clone(), index.clone()],
                    ),
                    node,
                );
                let mapped = builder.callback(&transform, vec![item], node)?;
                builder.code.push(Statement::Evaluate(
                    Rhs::Builtin(
                        "tensor.setFlat".into(),
                        Ty::Unit,
                        vec![output.atom.clone(), index, mapped.atom],
                    ),
                    builder.location(node),
                ));
                Ok(())
            })?;
            return Ok(output);
        }
        if method == "shape" {
            let source = values.remove(0);
            let rank = self.temporary(
                Ty::Int,
                Rhs::Builtin("tensor.rank".into(), Ty::Unit, vec![source.atom.clone()]),
                node,
            );
            let Ty::Record(id) = result else {
                return self.unsupported(node, "tensor shape array");
            };
            let output = self.temporary(
                result,
                Rhs::Builtin(
                    format!("arrayNew.{id}"),
                    Ty::Int,
                    vec![rank.atom.clone(), Atom::Int(0)],
                ),
                node,
            );
            self.tensor_loop(rank.atom, node, |builder, index| {
                let dimension = builder.temporary(
                    Ty::Int,
                    Rhs::Builtin(
                        "tensor.dimension".into(),
                        Ty::Unit,
                        vec![source.atom, index.clone()],
                    ),
                    node,
                );
                builder.code.push(Statement::Store(
                    Atom::ArrayItem(Box::new(output.atom.clone()), Box::new(index), Ty::Int),
                    Ty::Int,
                    Rhs::Copy(dimension.atom),
                    builder.location(node),
                ));
                Ok(())
            })?;
            return Ok(output);
        }
        if matches!(method, "sum" | "mean" | "min" | "max") {
            let total = values.len() == 1;
            let mut atoms = values.into_iter().map(|v| v.atom).collect::<Vec<_>>();
            if total {
                atoms.push(Atom::Int(-1));
            }
            atoms.push(Atom::Bool(total));
            return Ok(self.temporary(
                result,
                Rhs::Builtin(format!("tensor.{op}"), Ty::Unit, atoms),
                node,
            ));
        }
        let atoms = values.into_iter().map(|v| v.atom).collect();
        Ok(self.temporary(
            result,
            Rhs::Builtin(format!("tensor.{op}"), Ty::Unit, atoms),
            node,
        ))
    }
    fn tensor_loop(
        &mut self,
        length: Atom,
        node: &Node,
        body: impl FnOnce(&mut Self, Atom) -> Result<()>,
    ) -> Result<()> {
        let index = self.local(Ty::Int, false, false);
        self.code.push(Statement::Set(
            index,
            Rhs::Copy(Atom::Int(0)),
            self.location(node),
        ));
        let (condition, condition_code) = self.isolated(|builder| {
            Ok(builder.temporary(
                Ty::Bool,
                Rhs::Binary("<".into(), Ty::Int, Atom::Slot(index), length),
                node,
            ))
        })?;
        let (_, body) = self.isolated(|builder| {
            body(builder, Atom::Slot(index))?;
            Ok(Value::unit())
        })?;
        let step = vec![Statement::Set(
            index,
            Rhs::Binary("+".into(), Ty::Int, Atom::Slot(index), Atom::Int(1)),
            self.location(node),
        )];
        let id = self.next_loop;
        self.next_loop += 1;
        self.code.push(Statement::Loop {
            id,
            condition: condition_code,
            value: condition.atom,
            body,
            step,
        });
        Ok(())
    }
}
