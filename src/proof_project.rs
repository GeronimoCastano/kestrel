//! Check class-method assumptions against every class loaded by the program,
//! including private descendants and modules that do not import the theorem.
use super::*;

pub(crate) fn check_project_dispatches(modules: &mut [crate::modules::ModuleCheck]) {
    let mut definitions = BTreeMap::new();
    for module in modules.iter() {
        for (name, definition) in &module.checked.environment.definitions {
            definitions
                .entry(name.clone())
                .or_insert_with(|| definition.clone());
        }
    }
    // An owning module has the complete declaration, including private members.
    for module in modules.iter() {
        for (name, definition) in &module.checked.environment.definitions {
            if definition.file == module.parsed.source.name {
                definitions.insert(name.clone(), definition.clone());
            }
        }
    }
    for module in modules {
        if let Some(error) = dispatch_override_error(
            &module.checked.state_proof_dispatches,
            &definitions,
            module.parsed.root.span,
        ) {
            module.checked.diagnostics.push(error);
        }
        for (id, exported) in &module.checked.environment.proofs.theorems {
            if id.file != module.parsed.source.name || exported.theorem.dispatches.is_empty() {
                continue;
            }
            if let Some(error) =
                dispatch_override_error(&exported.theorem.dispatches, &definitions, id.span)
                && !module
                    .checked
                    .diagnostics
                    .iter()
                    .any(|d| d.code == error.code && d.span == error.span)
            {
                module.checked.diagnostics.push(error);
            }
        }
    }
}
