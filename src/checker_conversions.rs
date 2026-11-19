impl Checker<'_> {
    fn conversion_call(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        let callee = ungroup(node.children.first()?);
        let arguments = node.children.iter().skip(1).collect::<Vec<_>>();
        self.conversion_arguments(node, callee, &arguments, expected)
    }

    /// A bare conversion name on the right of `|>` receives the piped value:
    /// `1 |> double` and `[0, 0] |> Tensor`. Conversion syntax has no value of
    /// its own, but a pipe call is still a direct call with one argument.
    fn conversion_pipe(&mut self, node: &Node, expected: Option<&Type>) -> Option<Type> {
        let callee = ungroup(node.children.get(2)?);
        if callee.kind != K::Name {
            return None;
        }
        let value = ungroup(&node.children[0]);
        self.conversion_arguments(node, callee, &[value], expected)
    }

    fn conversion_arguments(
        &mut self,
        node: &Node,
        callee: &Node,
        arguments: &[&Node],
        expected: Option<&Type>,
    ) -> Option<Type> {
        let head = if callee.kind == K::BracketApplication {
            &callee.children[0]
        } else {
            callee
        };
        if !matches!(head.kind, K::Name | K::Member) {
            return None;
        }
        let written = if head.kind == K::Member {
            self.text(&head.children[1])
        } else {
            self.text(head)
        };
        if !matches!(written, "Tensor" | "Array" | "double" | "String") {
            return None;
        }
        // The existing two-argument Array(size, fill) keeps its constructor path.
        if written == "Array" && arguments.len() != 1 {
            return None;
        }
        let constructor = self.infer(callee, None);
        let Type::Constructor(name, type_args) = constructor else {
            return None;
        };
        if !matches!(name.as_str(), "Tensor" | "Array" | "double" | "String") {
            return None;
        }
        let raw = arguments
            .iter()
            .map(|argument| {
                let argument = ungroup(argument);
                // A call wraps each value in Argument or NamedArgument; a pipe
                // hands over the value itself.
                let (name, value) = if matches!(argument.kind, K::Argument | K::NamedArgument) {
                    (
                        (argument.kind == K::NamedArgument)
                            .then(|| self.text(&argument.children[0]).to_owned()),
                        argument.children.last().unwrap(),
                    )
                } else {
                    (None, argument)
                };
                (name, Type::Invalid, value, false)
            })
            .collect::<Vec<_>>();
        let labels = if name == "Tensor" && raw.len() == 2 {
            vec!["values", "shape"]
        } else {
            vec![if name == "Array" || name == "Tensor" {
                "values"
            } else {
                "value"
            }]
        };
        let Type::Function(mut skeleton) =
            Type::function(vec![Type::Invalid; labels.len()], Type::Invalid)
        else {
            unreachable!()
        };
        for (p, label) in skeleton.parameters.iter_mut().zip(&labels) {
            p.name = Some((*label).into());
        }
        let Some(order) = argument_order(&skeleton, &raw) else {
            self.error(
                node,
                "E_CALL_ARGUMENTS",
                format!("{name} has invalid conversion arguments."),
                "Pass values and an optional tensor shape, or one value for double/String.",
            );
            return Some(Type::Invalid);
        };
        let ordered = order
            .into_iter()
            .map(|index| &raw[index.unwrap()])
            .collect::<Vec<_>>();
        let input = ordered[0].2;
        let element = if name == "Tensor" {
            Type::named("double")
        } else {
            type_args
                .first()
                .cloned()
                .or_else(|| match expected.map(|ty| self.inference.resolve(ty)) {
                    Some(Type::Named(n, args)) if n == "Array" => args.first().cloned(),
                    _ => None,
                })
                .unwrap_or_else(|| self.inference.fresh())
        };
        let hint = (ungroup(input).kind == K::List && matches!(name.as_str(), "Array" | "Tensor"))
            .then(|| Type::applied("List", vec![element.clone()]));
        let source = self.infer(input, hint.as_ref());
        let source = self.inference.resolve(&source);
        let mut types = vec![source.clone()];
        let mut arguments = vec![(ordered[0].0.clone(), source.clone(), input, false)];
        let mut effects = false;
        let mut passing = Passing::Borrow;
        let (result, operation) = match name.as_str() {
            "Tensor" => {
                if !matches!(&source, Type::Named(n, args) if matches!(n.as_str(), "List" | "Array") && args == &[Type::named("double")])
                {
                    self.error(
                        input,
                        "E_TENSOR_ELEMENT",
                        "Tensor(values) needs List[double] or Array[double].",
                        "Use double elements, with explicit numeric conversions where needed.",
                    );
                    return Some(Type::Invalid);
                }
                let known_length =
                    (ungroup(input).kind == K::List).then(|| ungroup(input).children.len() as u64);
                let shape = if ordered.len() == 2 {
                    let written = ordered[1].2;
                    let ty = self.infer(written, None);
                    if !(matches!(&ty, Type::Tuple(parts) if parts.iter().all(|t| *t == Type::named("Int")))
                        || matches!(&ty, Type::Named(name, parts) if name == "Array" && parts == &[Type::named("Int")]))
                    {
                        self.error(
                            written,
                            "E_TENSOR_SHAPE",
                            "Tensor shape must be a tuple of Int dimensions or Array[Int].",
                            "Write shape = (rows, columns), or (length,) for one dimension.",
                        );
                        return Some(Type::Invalid);
                    }
                    types.push(ty.clone());
                    arguments.push((ordered[1].0.clone(), ty, written, false));
                    let shape = (ungroup(written).kind == K::Tuple)
                        .then(|| {
                            ungroup(written)
                                .children
                                .iter()
                                .map(|d| tensor_literal(self.source, d).map(Index::Number))
                                .collect::<Option<Vec<_>>>()
                        })
                        .flatten();
                    if let (Some(length), Some(shape)) = (known_length, shape.as_ref())
                        && tensor_element_counts_match(&[Index::Number(length)], shape)
                            == Some(false)
                    {
                        self.error(
                            node,
                            "E_TENSOR_SHAPE",
                            "Tensor shape does not match the number of elements.",
                            "Choose dimensions whose product equals the list length.",
                        );
                    }
                    shape
                } else {
                    known_length.map(|length| vec![Index::Number(length)])
                };
                (crate::types::tensor_type(shape), "std.tensor.construct")
            }
            "Array" => {
                let Some(Type::Function(method)) = self.builtin_member(&source, "toArray") else {
                    self.error(
                        input,
                        "E_TO_ARRAY",
                        "Array(values) needs an Iterable of copyable elements.",
                        "Pass a List or another value supported by toArray().",
                    );
                    return Some(Type::Invalid);
                };
                effects = method.effects;
                passing = if method.receiver_mutates {
                    Passing::Mut
                } else {
                    Passing::Borrow
                };
                let result = *method.result;
                if !type_args.is_empty() {
                    self.require(node, &Type::applied("Array", type_args), &result);
                }
                (result, "std.convert.toArray")
            }
            "double" => {
                if !matches!(&source, Type::Named(n, args) if args.is_empty() && matches!(n.as_str(), "Int" | "Nat" | "double"))
                {
                    self.error(
                        input,
                        "E_CONVERSION",
                        "double(value) accepts Int, Nat or double.",
                        "Use parseDouble for text and checkedToDouble for BigInt.",
                    );
                    return Some(Type::Invalid);
                }
                (Type::named("double"), "std.convert.toDouble")
            }
            _ => {
                if !self.satisfy_trait(&source, &Type::named("Text")) {
                    self.error(
                        input,
                        "E_TRAIT_BOUND",
                        format!("String(value) requires {source} to provide Text."),
                        "Declare has Text and implement text() for this type.",
                    );
                    return Some(Type::Invalid);
                }
                (Type::named("String"), "std.convert.text")
            }
        };
        let Type::Function(mut function) = Type::function(types, result) else {
            unreachable!()
        };
        for (p, label) in function.parameters.iter_mut().zip(labels) {
            p.name = Some(label.into());
        }
        function.parameters[0].passing = passing;
        function.effects = effects;
        function.callable = Some(Callable::Builtin(operation.into()));
        arguments.sort_by_key(|a| a.2.span.start);
        let result = self.apply(node, Type::Function(function), &arguments, expected);
        if name == "Array" {
            self.note_to_array(node, &result);
        }
        Some(result)
    }
}
