// Lowering for checked Variable calls. The method name alone decides the C
// helper; `native_variable.rs` has the complete table.
impl Builder<'_, '_> {
    fn variable_index_value(&mut self, node: &Node) -> Result<Value> {
        let owner = self.expression(&node.children[0])?;
        let parts = if node.children[1].kind == K::Tuple {
            node.children[1].children.iter().collect::<Vec<_>>()
        } else {
            vec![&node.children[1]]
        };
        let mut atoms = vec![owner.atom];
        for part in &parts {
            if part.kind == K::Slice {
                return self.unsupported(node, "a Variable slice");
            }
            let value = self.expression(part)?;
            let value = self.freeze(value, part)?;
            atoms.push(value.atom);
        }
        Ok(self.temporary(
            Ty::Variable,
            Rhs::Builtin("variable.index".into(), Ty::Unit, atoms),
            node,
        ))
    }

    fn variable_call_value(
        &mut self,
        method: &str,
        values: Vec<Value>,
        result: Ty,
        node: &Node,
    ) -> Result<Value> {
        let atoms = values.into_iter().map(|value| value.atom).collect();
        Ok(self.temporary(
            result,
            Rhs::Builtin(format!("variable.{method}"), Ty::Unit, atoms),
            node,
        ))
    }
}
