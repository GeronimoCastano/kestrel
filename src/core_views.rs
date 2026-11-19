//! An escaping class or trait view may not leave newly owned return storage behind.
use super::*;

type BoxSlot = (usize, Slot);

impl Compiler<'_> {
    pub(super) fn validate_view_escapes(&self) -> Result<()> {
        if self.view_escapes.is_empty() {
            return Ok(());
        }
        let mut owned = BTreeSet::new();
        let mut edges: BTreeMap<BoxSlot, Vec<BoxSlot>> = BTreeMap::new();
        let mut outputs = BTreeMap::new();
        for (id, function) in self.functions.iter().enumerate() {
            let function = function.as_ref().unwrap();
            let out = if matches!(function.result, Ty::Ref(_)) {
                function
                    .parameters
                    .last()
                    .filter(|(_, ty, passing)| {
                        matches!(ty, Ty::Boxed(_)) && *passing == Passing::Mut
                    })
                    .map(|(slot, _, _)| *slot)
            } else {
                None
            };
            if let Some(slot) = out {
                outputs.insert(id, (id, slot));
            }
            for (slot, ty, _) in &function.parameters {
                if matches!(ty, Ty::Boxed(_)) && Some(*slot) != out {
                    owned.insert((id, *slot));
                }
            }
            visit(&function.body, &mut |statement| {
                let rhs = match statement {
                    Statement::Set(slot, rhs, _)
                    | Statement::Store(Atom::Slot(slot), _, rhs, _) => {
                        if !slot.global && matches!(function.locals[slot.id].ty, Ty::Boxed(_)) {
                            match rhs {
                                Rhs::Copy(Atom::Slot(source)) | Rhs::Take(Atom::Slot(source))
                                    if !source.global =>
                                {
                                    edges.entry((id, *source)).or_default().push((id, *slot));
                                }
                                _ => {
                                    owned.insert((id, *slot));
                                }
                            }
                        }
                        Some(rhs)
                    }
                    Statement::Evaluate(rhs, _) | Statement::Store(_, _, rhs, _) => Some(rhs),
                    Statement::Spawn { call, .. } => Some(call),
                    _ => None,
                };
                if let Some(Rhs::Call(callee, args)) = rhs {
                    for ((formal, ty, mode), (actual, _)) in self.functions[*callee]
                        .as_ref()
                        .unwrap()
                        .parameters
                        .iter()
                        .zip(args)
                    {
                        if matches!(ty, Ty::Boxed(_))
                            && *mode == Passing::Mut
                            && let Atom::Slot(slot) = actual
                        {
                            edges
                                .entry((*callee, *formal))
                                .or_default()
                                .push((id, *slot));
                        }
                    }
                }
            });
        }
        let mut pending = owned.iter().copied().collect::<Vec<_>>();
        while let Some(source) = pending.pop() {
            for target in edges.get(&source).into_iter().flatten() {
                if owned.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        for (module, span, calls) in &self.view_escapes {
            if calls
                .iter()
                .any(|call| outputs.get(call).is_none_or(|slot| owned.contains(slot)))
            {
                return Err(LocatedDiagnostic {module:*module,diagnostic:Diagnostic::new("E_NATIVE_OWNERSHIP","This view can retain a newly owned result whose storage would end with this function.",*span).with_hint("Return an owned Box[dyn Trait], or keep this view inside the function that owns the result.")});
            }
        }
        Ok(())
    }
}

fn visit(statements: &[Statement], callback: &mut impl FnMut(&Statement)) {
    for statement in statements {
        callback(statement);
        match statement {
            Statement::If(_, a, b) => {
                visit(a, callback);
                visit(b, callback);
            }
            Statement::Loop {
                condition,
                body,
                step,
                ..
            } => {
                visit(condition, callback);
                visit(body, callback);
                visit(step, callback);
            }
            Statement::Match(_, arms) => {
                for arm in arms {
                    visit(&arm.test, callback);
                    visit(&arm.bindings, callback);
                    if let Some((guard, _)) = &arm.guard {
                        visit(guard, callback);
                    }
                    visit(&arm.body, callback);
                    visit(&arm.cleanup, callback);
                }
            }
            _ => {}
        }
    }
}
