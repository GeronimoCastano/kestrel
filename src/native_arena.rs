//! Owned arena storage. Links carry identities, never pointers into the storage.
use super::{c_type, suffix, union};
use crate::{
    core::{LayoutKind, Program, Ty},
    types::Type,
};
use std::fmt::Write;

pub(super) fn emit(program: &Program, id: usize, t: Ty, out: &mut String) {
    let c = c_type(t);
    let Type::Named(_, items) = &program.layouts[id].source else {
        unreachable!()
    };
    let copyable = crate::types::copies(&items[0]);
    writeln!(out, "static KRecord_{id} k_resource_{id}_new(void) {{ struct KResourceData_{id} *h = calloc(1,sizeof *h); if (!h) k_fatal(\"Out of memory\"); h->identity = k_arena_new_identity(); return (KRecord_{id}) {{h}}; }}").unwrap();
    writeln!(out, "static KArenaRef k_resource_{id}_insert(KRecord_{id} *arena, {c} owned) {{ struct KResourceData_{id} *h = arena->handle; if (!h) k_fatal(\"Arena was moved\"); if (h->length == h->capacity) {{ if (h->capacity > SIZE_MAX / 2 / sizeof *h->items) k_fatal(\"Arena is too large\"); size_t capacity = h->capacity ? h->capacity * 2 : 8; if (capacity > SIZE_MAX / sizeof *h->items) k_fatal(\"Arena is too large\"); void *items = realloc(h->items,capacity * sizeof *h->items); if (!items) k_fatal(\"Out of memory\"); h->items = items; h->capacity = capacity; }} size_t slot = h->length++; h->items[slot] = owned; return (KArenaRef) {{h->identity, slot}}; }}").unwrap();
    if let Some((result, success, failure)) = union(program, &Type::named("void")) {
        let release = if t.owns() {
            format!("k_{}_drop(&owned);", suffix(t))
        } else {
            String::new()
        };
        let drop_old = if t.owns() {
            format!("k_{}_drop(&h->items[key.slot]);", suffix(t))
        } else {
            String::new()
        };
        writeln!(out, "static KRecord_{result} k_resource_{id}_replace(KRecord_{id} *arena, KArenaRef key, {c} owned) {{ struct KResourceData_{id} *h = arena->handle; if (!h || key.arena != h->identity || key.slot >= h->length) {{ {release} return k_aggregate_{result}_{failure}(k_runtime_error(1,K_TEXT(\"Arena handle does not belong to this arena\"))); }} {drop_old} h->items[key.slot] = owned; return k_aggregate_{result}_{success}(); }}").unwrap();
    }
    let result_ty = match t {
        Ty::Record(value) if !copyable => Ty::Ref(value),
        Ty::Boxed(value) => Ty::Ref(value),
        Ty::DynBox(value) => Ty::Dyn(value),
        other => other,
    };
    for (option, layout) in program.layouts.iter().enumerate() {
        let (LayoutKind::Enum(cases) | LayoutKind::Step(cases)) = &layout.kind else {
            continue;
        };
        if !matches!(&layout.source, Type::Named(name, _) if name == "Option")
            || cases[0].fields != [result_ty]
        {
            continue;
        }
        let value = if matches!(t, Ty::Record(_)) && matches!(result_ty, Ty::Ref(_)) {
            "&h->items[key.slot]".into()
        } else if super::super::copies(program, t) || t == Ty::Python {
            format!("k_{}_copy(h->items[key.slot])", suffix(t))
        } else {
            "h->items[key.slot]".into()
        };
        writeln!(out, "static KRecord_{option} k_resource_{id}_get_{option}(KRecord_{id} *arena, KArenaRef key) {{ struct KResourceData_{id} *h = arena->handle; if (!h || key.arena != h->identity || key.slot >= h->length) return k_aggregate_{option}_1(); return k_aggregate_{option}_0({value}); }}").unwrap();
    }
}
