//! Finds temporaries that a new value may take over instead of copying.
//!
//! A copyable enum or record is copied wherever it is read (Q169). When the value sits in a
//! temporary that nothing else reads, the copy and the later drop cancel out: the copy adds a
//! reference to each boxed part and the drop removes it again. Taking the temporary gives the
//! same result with neither step.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// The loops a statement sits inside, outermost first.
type LoopPath = Vec<usize>;

#[derive(Default)]
struct Uses {
    /// Every place a slot is named, except where it is assigned or dropped.
    mentions: BTreeMap<Slot, usize>,
    /// The loops around each assignment to a slot.
    sets: BTreeMap<Slot, Vec<LoopPath>>,
    /// The loops around each place a slot is itself a part of a new value.
    parts: BTreeMap<Slot, Vec<LoopPath>>,
}

/// Local temporaries whose one reader is a new value built in the same loop as every
/// assignment to them. Each run of that reader follows an assignment, and nothing reads the
/// slot afterwards, so the reader may empty it.
pub(super) fn consumable(program: &Program, function: &Function) -> BTreeSet<Slot> {
    let mut uses = Uses::default();
    statements(&function.body, &mut vec![], &mut uses);
    uses.parts
        .iter()
        .filter(|(slot, parts)| {
            let Some(local) = (!slot.global)
                .then(|| function.locals.get(slot.id))
                .flatten()
            else {
                return false;
            };
            local.owned
                && !local.parameter
                && matches!(local.ty, Ty::Record(id) if program.copyable_layouts.contains(&id))
                && parts.len() == 1
                && uses.mentions.get(slot) == Some(&1)
                && uses
                    .sets
                    .get(slot)
                    .is_some_and(|sets| sets.iter().all(|path| *path == parts[0]))
        })
        .map(|(slot, _)| *slot)
        .collect()
}

fn mention(slot: Slot, uses: &mut Uses) {
    *uses.mentions.entry(slot).or_default() += 1;
}

fn atom(atom: &Atom, uses: &mut Uses) {
    match atom {
        Atom::Slot(slot) => mention(*slot, uses),
        Atom::Int(_)
        | Atom::Nat(_)
        | Atom::Double(_)
        | Atom::Bool(_)
        | Atom::Text(_)
        | Atom::Unit => {}
        Atom::NatPred(inner)
        | Atom::Field(inner, ..)
        | Atom::Address(inner, _)
        | Atom::Payload(inner, ..)
        | Atom::Tag(inner)
        | Atom::ErrorType(inner)
        | Atom::ErrorPayload(inner, _)
        | Atom::ResourceValue(inner, _)
        | Atom::ArrayLength(inner)
        | Atom::CollectionLength(inner)
        | Atom::LengthLess(inner, _)
        | Atom::MapCapacity(inner)
        | Atom::StepOwner(inner)
        | Atom::OwnerValue(inner, ..)
        | Atom::EnvField(inner, ..) => self::atom(inner, uses),
        Atom::ListItem(a, b, _)
        | Atom::ArrayItem(a, b, _)
        | Atom::MapOccupied(a, b)
        | Atom::MapItem(a, b, _)
        | Atom::SetItem(a, b, ..) => {
            self::atom(a, uses);
            self::atom(b, uses);
        }
    }
}

fn test(test: &Test, uses: &mut Uses) {
    match test {
        Test::Yes => {}
        Test::Equal(_, a, b) => {
            atom(a, uses);
            atom(b, uses);
        }
        Test::Less(_, a, b) => {
            atom(a, uses);
            atom(b, uses);
        }
        Test::BigIntLiteral(a, _) | Test::ListLength(a, ..) => atom(a, uses),
        Test::And(a, b) | Test::Or(a, b) => {
            self::test(a, uses);
            self::test(b, uses);
        }
        Test::Not(inner) => self::test(inner, uses),
    }
}

fn rhs(rhs: &Rhs, path: &LoopPath, uses: &mut Uses) {
    match rhs {
        Rhs::StaticRead(_) => {}
        Rhs::Copy(a)
        | Rhs::Take(a)
        | Rhs::StaticWrite(_, a)
        | Rhs::Unary(_, a)
        | Rhs::Text(_, a, _)
        | Rhs::ErrorBox(_, a, _)
        | Rhs::FileClose(a)
        | Rhs::OwnerNew(a, _)
        | Rhs::MakeDyn(_, _, a)
        | Rhs::DynBoxNew(_, _, a, _)
        | Rhs::BoxNew(_, a, _)
        | Rhs::BoxUnwrap(_, a)
        | Rhs::ListSlice(_, a, ..) => atom(a, uses),
        Rhs::Binary(_, _, a, b)
        | Rhs::ListConcat(_, a, b)
        | Rhs::ListAppend(_, a, b, _)
        | Rhs::StepSome(_, a, _, b)
        | Rhs::MapGet(_, a, b, _) => {
            atom(a, uses);
            atom(b, uses);
        }
        Rhs::ListAppendFrom(_, a, b, c)
        | Rhs::ListAppendSet(_, _, a, b, c)
        | Rhs::ListAppendOwner(_, a, b, _, c)
        | Rhs::MapPut(_, a, b, _, c, _) => {
            atom(a, uses);
            atom(b, uses);
            atom(c, uses);
        }
        Rhs::Call(_, arguments) => arguments.iter().for_each(|(a, _)| atom(a, uses)),
        Rhs::Builtin(_, _, atoms) | Rhs::Join(atoms) | Rhs::Python(_, atoms) => {
            atoms.iter().for_each(|a| atom(a, uses))
        }
        Rhs::Aggregate(_, _, values) => {
            for (value, _) in values {
                if let Atom::Slot(slot) = value {
                    uses.parts.entry(*slot).or_default().push(path.clone());
                }
                atom(value, uses);
            }
        }
        Rhs::ListNew(_, values) | Rhs::MapNew(_, values) => {
            values.iter().for_each(|(a, _)| atom(a, uses))
        }
        Rhs::Test(inner) => test(inner, uses),
        Rhs::JoinTasks(slot) => mention(*slot, uses),
        Rhs::DynCall(_, _, receiver, arguments) | Rhs::ClosureCall(_, receiver, arguments) => {
            atom(receiver, uses);
            arguments.iter().for_each(|a| atom(a, uses));
        }
        Rhs::ClosureNew(_, _, captured) => {
            for (a, _, _) in captured.iter().flat_map(|(_, values)| values) {
                atom(a, uses);
            }
        }
        Rhs::StringAppend(slot, a) | Rhs::StringAppendText(slot, _, a) => {
            mention(*slot, uses);
            atom(a, uses);
        }
    }
}

fn cleanups(cleanups: &[Cleanup], uses: &mut Uses) {
    for cleanup in cleanups {
        match cleanup {
            Cleanup::Drop(_) => {}
            Cleanup::File(a) => atom(a, uses),
            Cleanup::Join(slot, _) => mention(*slot, uses),
        }
    }
}

fn statements(code: &[Statement], path: &mut LoopPath, uses: &mut Uses) {
    for statement in code {
        match statement {
            Statement::Set(slot, value, _) => {
                uses.sets.entry(*slot).or_default().push(path.clone());
                rhs(value, path, uses);
            }
            Statement::Store(place, _, value, _) => {
                atom(place, uses);
                rhs(value, path, uses);
            }
            Statement::Evaluate(value, _) => rhs(value, path, uses),
            Statement::Print(_, a) => atom(a, uses),
            Statement::If(condition, then, otherwise) => {
                atom(condition, uses);
                statements(then, path, uses);
                statements(otherwise, path, uses);
            }
            Statement::Match(_, arms) => {
                for arm in arms {
                    statements(&arm.test, path, uses);
                    atom(&arm.condition, uses);
                    statements(&arm.bindings, path, uses);
                    if let Some((guard, condition)) = &arm.guard {
                        statements(guard, path, uses);
                        atom(condition, uses);
                    }
                    statements(&arm.body, path, uses);
                    statements(&arm.cleanup, path, uses);
                }
            }
            Statement::Loop {
                id,
                condition,
                value,
                body,
                step,
            } => {
                path.push(*id);
                statements(condition, path, uses);
                atom(value, uses);
                statements(body, path, uses);
                statements(step, path, uses);
                path.pop();
            }
            Statement::Break(_, pending) | Statement::Continue(_, pending) => {
                cleanups(pending, uses)
            }
            Statement::Return(a, pending) => {
                atom(a, uses);
                cleanups(pending, uses);
            }
            Statement::TailCall(_, arguments) => arguments.iter().for_each(|(a, _)| atom(a, uses)),
            Statement::Drop(_) => {}
            Statement::Spawn { scope, call, .. } => {
                mention(*scope, uses);
                rhs(call, path, uses);
            }
        }
    }
}
