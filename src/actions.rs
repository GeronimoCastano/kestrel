//! Editor quick fixes for code that is still missing.
//!
//! The checker knows exactly what an incomplete declaration lacks: the methods
//! a trait requires, the cases a `match` leaves uncovered, and the cases a proof
//! split omits. It records each fix beside the diagnostic that explains the
//! gap, and `kestrel serve` hands the edits to the editor.
//!
//! A fix inserts boilerplate. Bodies are empty blocks with a TODO comment, so
//! the new code still reports what the programmer has to write.

use crate::{
    source::{SourceFile, Span},
    syntax::{Node, SyntaxKind as K},
    types::{FunctionType, Passing, Type},
};
use std::collections::BTreeSet;

/// One replacement in the checked text. An insertion has an empty span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Edit {
    pub span: Span,
    pub text: String,
}

/// A quick fix the editor can apply as one action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fix {
    pub title: String,
    pub edits: Vec<Edit>,
}

/// Where text goes when it is added just before a node's closing brace.
struct Insertion {
    at: usize,
    prefix: &'static str,
    suffix: String,
}

fn insertion_before_close(source: &SourceFile, node: &Node, indent: &str) -> Option<Insertion> {
    let text = &source.text;
    let close = text.get(node.span.range())?.rfind('}')? + node.span.start;
    let line_start = text[..close].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // When the brace stands alone, insert before its indentation so the added
    // lines keep the brace's own column. An inline brace needs a new line.
    if text[line_start..close].trim().is_empty() {
        Some(Insertion {
            at: line_start,
            prefix: "",
            suffix: "\n".into(),
        })
    } else {
        Some(Insertion {
            at: close,
            prefix: "\n",
            suffix: format!("\n{indent}"),
        })
    }
}

fn line_indent(text: &str, offset: usize) -> &str {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line = &text[start..];
    let end = line
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(line.len());
    &line[..end]
}

/// The indentation of a block's arms: the first arm's own column when it starts
/// on a later line, otherwise one level below the block.
fn arm_indent(source: &SourceFile, node: &Node) -> String {
    let node_line = source.text[..node.span.start]
        .bytes()
        .filter(|b| *b == b'\n')
        .count();
    if let Some(arm) = node.children.iter().find(|child| child.kind == K::MatchArm) {
        let arm_line = source.text[..arm.span.start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
        if arm_line > node_line {
            return line_indent(&source.text, arm.span.start).to_owned();
        }
    }
    format!("{}    ", line_indent(&source.text, node.span.start))
}

/// Adds arms before the closing brace of a `match`, `induction` or proof split.
fn append_arms(source: &SourceFile, node: &Node, arms: &[String], indent: &str) -> Edit {
    let text = &source.text;
    let close = text[node.span.range()]
        .rfind('}')
        .map(|i| i + node.span.start)
        .unwrap_or(node.span.end);
    let lines = arms
        .iter()
        .map(|arm| format!("{indent}{arm},"))
        .collect::<Vec<_>>()
        .join("\n");
    let last = node
        .children
        .iter()
        .rfind(|child| child.kind == K::MatchArm);
    if let Some(last) = last {
        // A comma in a trailing comment does not separate arms.
        let has_comma = text[last.span.end..close]
            .lines()
            .any(|line| line.split("//").next().unwrap_or("").contains(','));
        if !has_comma {
            // The previous arm has no trailing comma, so add one before the
            // new line instead of starting the next arm with a comma.
            return Edit {
                span: Span::new(last.span.end, last.span.end),
                text: format!(",\n{lines}"),
            };
        }
    }
    let node_indent = line_indent(text, node.span.start);
    if let Some(at) = insertion_before_close(source, node, node_indent) {
        Edit {
            span: Span::new(at.at, at.at),
            text: format!("{}{lines}{}", at.prefix, at.suffix),
        }
    } else {
        Edit {
            span: Span::new(close, close),
            text: format!("\n{lines}"),
        }
    }
}

fn base_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Named(name, _) => Some(name.rsplit('.').next().unwrap_or(name)),
        Type::Borrow(inner) | Type::StoredView(inner) => base_name(inner),
        Type::Indexed(inner, _) => base_name(inner),
        _ => None,
    }
}

fn same_type(left: &Type, right: &Type) -> bool {
    match (base_name(left), base_name(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

/// The proof kernel recognizes a recursive field by its exact enum identity.
fn same_enum(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Named(..), Type::Named(..)) => left == right,
        _ => false,
    }
}

/// Binder names for a case's fields. Recursive fields read as the smaller
/// value (`k` for Nat, `rest` for a list or chain); other fields take ordinary
/// value names.
fn binders(fields: &[Type], owner: &Type, induction: bool) -> Vec<String> {
    if base_name(owner) == Some("List") && fields.len() == 2 && fields[1] == *owner {
        return vec!["value".into(), "rest".into()];
    }
    let recursive = fields
        .iter()
        .map(|field| same_type(field, owner))
        .collect::<Vec<_>>();
    match fields.len() {
        0 => vec![],
        1 if recursive[0] => {
            if induction {
                if base_name(owner) == Some("Nat") {
                    vec!["k".into()]
                } else {
                    vec!["rest".into()]
                }
            } else if base_name(owner) == Some("Nat") {
                vec!["k".into()]
            } else {
                vec!["value".into()]
            }
        }
        1 => vec!["value".into()],
        2 => match (recursive[0], recursive[1]) {
            (true, false) => vec!["rest".into(), "value".into()],
            (false, true) => vec!["value".into(), "rest".into()],
            _ => vec!["left".into(), "right".into()],
        },
        _ => (0..fields.len())
            .map(|index| format!("value{}", index + 1))
            .collect(),
    }
}

/// One case pattern for a missing case, or `None` when the compiler's internal
/// case name has no surface spelling (`error:other`, a union alternative).
fn case_pattern(case: &str, fields: &[Type], owner: &Type) -> Option<String> {
    match case {
        "error:other" => return None,
        "union:0" | "union:1" | "union:2" | "union:3" => return None,
        "list:nil" => return Some("[]".into()),
        "list:cons" => {
            let element = fields.first()?;
            let name = binders(std::slice::from_ref(element), owner, false)
                .pop()
                .unwrap_or_else(|| "value".into());
            return Some(format!("[{name}, ..rest]"));
        }
        "tuple" => {
            let names = binders(fields, owner, false);
            return Some(format!("({})", names.join(", ")));
        }
        _ => {}
    }
    let name = case.rsplit('.').next().unwrap_or(case);
    if fields.is_empty() {
        Some(name.into())
    } else {
        let names = binders(fields, owner, false);
        Some(format!("{name}({})", names.join(", ")))
    }
}

/// Fixes for an ordinary `match`: one per missing case, plus one that adds all
/// of them. When the compiler cannot name a case (an open `Error` fallback),
/// the wildcard arm is the only safe skeleton.
pub(crate) fn match_fixes(
    source: &SourceFile,
    node: &Node,
    owner: &Type,
    missing: &[(String, Vec<Type>)],
) -> Vec<Fix> {
    let indent = arm_indent(source, node);
    // The open Error root has every standard and user error as a case. Enumerating
    // them would bury the one arm a reader wants: a wildcard fallback.
    if base_name(owner) == Some("Error") {
        let arm = "_ => { }".to_owned();
        return vec![Fix {
            title: "Add a wildcard case".into(),
            edits: vec![append_arms(source, node, &[arm], &indent)],
        }];
    }
    let mut fixes = vec![];
    let mut arms = vec![];
    for (case, fields) in missing {
        let Some(pattern) = case_pattern(case, fields, owner) else {
            continue;
        };
        let arm = format!("{pattern} => {{ }}");
        let edit = append_arms(source, node, std::slice::from_ref(&arm), &indent);
        fixes.push(Fix {
            title: format!("Add the `{pattern}` case"),
            edits: vec![edit],
        });
        arms.push(arm);
    }
    if arms.len() > 1 {
        fixes.push(Fix {
            title: format!("Add all {} missing cases", arms.len()),
            edits: vec![append_arms(source, node, &arms, &indent)],
        });
    }
    if fixes.is_empty() {
        let pattern = "_".to_owned();
        fixes.push(Fix {
            title: "Add a wildcard case".into(),
            edits: vec![append_arms(source, node, &[pattern], &indent)],
        });
    }
    fixes
}

/// Fixes for a `match` or `induction` inside a proof. The recursive case names
/// its induction hypothesis `ih`, exactly as the kernel expects.
pub(crate) fn proof_case_fixes(
    source: &SourceFile,
    node: &Node,
    owner: &Type,
    variants: &[(String, Vec<Type>)],
    missing: &[String],
    induction: bool,
) -> Vec<Fix> {
    let indent = arm_indent(source, node);
    let mut fixes = vec![];
    let mut arms = vec![];
    for name in missing {
        let Some((_, fields)) = variants.iter().find(|(case, _)| case == name) else {
            continue;
        };
        let Some(pattern) = case_pattern(name, fields, owner) else {
            continue;
        };
        let names = binders(fields, owner, false);
        let hypotheses = fields
            .iter()
            .zip(names)
            .filter(|(field, _)| same_enum(field, owner))
            .map(|(_, name)| format!("ih{name}"))
            .collect::<Vec<_>>();
        let lhs = if induction && !hypotheses.is_empty() {
            format!("{pattern} {}", hypotheses.join(" "))
        } else {
            pattern
        };
        let arm = format!("{lhs} => {{ }}");
        let edit = append_arms(source, node, std::slice::from_ref(&arm), &indent);
        fixes.push(Fix {
            title: format!("Add the `{lhs}` case"),
            edits: vec![edit],
        });
        arms.push(arm);
    }
    if arms.len() > 1 {
        fixes.push(Fix {
            title: format!("Add all {} missing cases", arms.len()),
            edits: vec![append_arms(source, node, &arms, &indent)],
        });
    }
    fixes
}

/// The indentation of a tactic block's branches.
fn tactic_branch_indent(source: &SourceFile, block: &Node) -> String {
    let block_line = source.text[..block.span.start]
        .bytes()
        .filter(|b| *b == b'\n')
        .count();
    if let Some(branch) = block
        .children
        .iter()
        .find(|child| child.kind == K::TacticBranch)
    {
        let branch_line = source.text[..branch.span.start]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
        if branch_line > block_line {
            return line_indent(&source.text, branch.span.start).to_owned();
        }
    }
    format!("{}    ", line_indent(&source.text, block.span.start))
}

fn insert_tactic_lines(source: &SourceFile, block: &Node, lines: &[String], indent: &str) -> Edit {
    let node_indent = line_indent(&source.text, block.span.start);
    let body = lines
        .iter()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(at) = insertion_before_close(source, block, node_indent) {
        Edit {
            span: Span::new(at.at, at.at),
            text: format!("{}{body}{}", at.prefix, at.suffix),
        }
    } else {
        Edit {
            span: Span::new(block.span.end, block.span.end),
            text: format!("\n{body}"),
        }
    }
}

/// Fixes for a `cases` or `induction` tactic. Tactics separate branches with
/// whitespace rather than commas, and the recursive case names its hypothesis.
pub(crate) fn tactic_case_fixes(
    source: &SourceFile,
    block: &Node,
    owner: &Type,
    variants: &[(String, Vec<Type>)],
    missing: &[String],
    induction: bool,
) -> Vec<Fix> {
    let indent = tactic_branch_indent(source, block);
    let mut fixes = vec![];
    let mut branches = vec![];
    for name in missing {
        let Some((_, fields)) = variants.iter().find(|(case, _)| case == name) else {
            continue;
        };
        let Some(pattern) = case_pattern(name, fields, owner) else {
            continue;
        };
        let names = binders(fields, owner, false);
        let hypotheses = fields
            .iter()
            .zip(names)
            .filter(|(field, _)| same_enum(field, owner))
            .map(|(_, name)| format!("ih{name}"))
            .collect::<Vec<_>>();
        let lhs = if induction && !hypotheses.is_empty() {
            format!("{pattern} {}", hypotheses.join(" "))
        } else {
            pattern
        };
        let branch = format!("{lhs} => {{ }}");
        fixes.push(Fix {
            title: format!("Add the `{lhs}` branch"),
            edits: vec![insert_tactic_lines(
                source,
                block,
                std::slice::from_ref(&branch),
                &indent,
            )],
        });
        branches.push(branch);
    }
    if branches.len() > 1 {
        fixes.push(Fix {
            title: format!("Add all {} missing branches", branches.len()),
            edits: vec![insert_tactic_lines(source, block, &branches, &indent)],
        });
    }
    fixes
}

/// Fixes for splitting an `or` assumption: `Left(p)` and `Right(q)`.
pub(crate) fn proof_or_fixes(source: &SourceFile, node: &Node, missing: &[&str]) -> Vec<Fix> {
    let indent = arm_indent(source, node);
    let arms = missing
        .iter()
        .map(|side| match *side {
            "Right" => "Right(q)".to_owned(),
            _ => "Left(p)".to_owned(),
        })
        .collect::<Vec<_>>();
    let bodies = arms
        .iter()
        .map(|arm| format!("{arm} => {{ }}"))
        .collect::<Vec<_>>();
    let mut fixes = arms
        .iter()
        .zip(&bodies)
        .map(|(arm, body)| Fix {
            title: format!("Add the `{arm}` case"),
            edits: vec![append_arms(
                source,
                node,
                std::slice::from_ref(body),
                &indent,
            )],
        })
        .collect::<Vec<_>>();
    if bodies.len() > 1 {
        fixes.push(Fix {
            title: format!("Add all {} missing cases", bodies.len()),
            edits: vec![append_arms(source, node, &bodies, &indent)],
        });
    }
    fixes
}

fn plain_type(ty: &Type, parameters: &[String]) -> bool {
    let mut plain = true;
    ty.map(&mut |part| {
        if let Type::Parameter(name) = part {
            plain &= parameters.contains(name);
        }
        None
    });
    plain
}

/// The signature of a method a trait requires, without its body. `None` means
/// this contract cannot be written from the information a fix has, so the
/// diagnostic keeps its plain hint.
fn method_signature(
    name: &str,
    contract: &FunctionType,
    is_static: bool,
    overrides: bool,
    class_parameters: &[String],
) -> Option<String> {
    if !contract.bounds.is_empty() || !contract.requirements.is_empty() {
        return None;
    }
    if !plain_type(&contract.result, class_parameters) {
        return None;
    }
    let mut used = contract
        .parameters
        .iter()
        .filter_map(|parameter| parameter.name.clone())
        .collect::<BTreeSet<_>>();
    let mut candidates = ["value", "other", "left", "right", "item", "arg"].into_iter();
    let mut parameters = vec![];
    for parameter in &contract.parameters {
        if parameter.default || !plain_type(&parameter.ty, class_parameters) {
            return None;
        }
        let parameter_name = match &parameter.name {
            Some(name) => name.clone(),
            None => loop {
                if let Some(candidate) = candidates.next() {
                    if used.insert(candidate.to_owned()) {
                        break candidate.to_owned();
                    }
                } else {
                    break format!("arg{}", parameters.len() + 1);
                }
            },
        };
        let passing = match parameter.passing {
            Passing::Borrow => "",
            Passing::Mut => "mut ",
            Passing::Move => "move ",
        };
        parameters.push(format!("{passing}{parameter_name} : {}", parameter.ty));
    }
    let mut signature = String::from(if overrides {
        "@override public "
    } else {
        "public "
    });
    if is_static {
        signature.push_str("static ");
    }
    if contract.receiver_mutates {
        signature.push_str("mut ");
    }
    signature.push_str("function ");
    signature.push_str(name);
    signature.push('(');
    signature.push_str(&parameters.join(", "));
    signature.push_str(") : ");
    signature.push_str(&contract.result.to_string());
    if contract.effects {
        signature.push_str(" with Effects");
    }
    Some(signature)
}

/// One missing trait method, its contract, and whether the trait declares it
/// static.
pub(crate) struct MissingMethod<'a> {
    pub name: &'a str,
    pub contract: &'a FunctionType,
    pub is_static: bool,
    pub overrides: bool,
}

/// Stubs for the methods a class still owes a trait. A single missing method
/// gets one action; several also get one action that writes them all.
pub(crate) fn trait_method_fixes(
    source: &SourceFile,
    class: &Node,
    trait_name: &str,
    class_parameters: &[String],
    missing: &[MissingMethod<'_>],
) -> Vec<Fix> {
    let indent = line_indent(&source.text, class.span.start);
    let body_indent = format!("{indent}    ");
    let trait_name = trait_name.rsplit('.').next().unwrap_or(trait_name);
    let mut blocks = vec![];
    let mut fixes = vec![];
    for method in missing {
        let Some(signature) = method_signature(
            method.name,
            method.contract,
            method.is_static,
            method.overrides,
            class_parameters,
        ) else {
            continue;
        };
        let block = format!(
            "{body_indent}{signature} {{\n{body_indent}    // TODO: implement\n{body_indent}}}"
        );
        let Some(at) = insertion_before_close(source, class, indent) else {
            continue;
        };
        fixes.push(Fix {
            title: format!(
                "Add the `{}` method required by `{trait_name}`",
                method.name
            ),
            edits: vec![Edit {
                span: Span::new(at.at, at.at),
                text: format!("{}{block}{}", at.prefix, at.suffix),
            }],
        });
        blocks.push(block);
    }
    if blocks.len() > 1
        && let Some(at) = insertion_before_close(source, class, indent)
    {
        fixes.push(Fix {
            title: format!(
                "Add all {} missing methods required by `{trait_name}`",
                blocks.len()
            ),
            edits: vec![Edit {
                span: Span::new(at.at, at.at),
                text: format!("{}{}{}", at.prefix, blocks.join("\n"), at.suffix),
            }],
        });
    }
    fixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::check_source;

    fn apply(text: &str, edit: &Edit) -> String {
        let mut result = text.to_owned();
        result.replace_range(edit.span.range(), &edit.text);
        result
    }

    fn fix_for<'a>(module: &'a crate::modules::ModuleCheck, code: &str) -> &'a Fix {
        module
            .checked
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .and_then(|diagnostic| diagnostic.fixes.first())
            .expect("a diagnostic with a quick fix")
    }

    #[test]
    fn a_recursive_nat_case_names_its_smaller_value() {
        let nat = Type::named("Nat");
        assert_eq!(
            case_pattern("Nat.Succ", std::slice::from_ref(&nat), &nat),
            Some("Succ(k)".into())
        );
    }

    #[test]
    fn a_missing_trait_method_becomes_a_stub() {
        let module = check_source(
            "-",
            "trait Shape { function area() : double }\nclass C { has Shape }\n",
        );
        let fix = fix_for(&module, "E_TRAIT_METHOD");
        assert!(fix.title.contains("area"));
        let text = &fix.edits[0].text;
        assert!(text.contains("public function area() : double"), "{text}");
        assert!(text.contains("// TODO: implement"), "{text}");
        let applied = apply(&module.parsed.source.text, &fix.edits[0]);
        assert!(
            applied.contains("    public function area() : double"),
            "{applied}"
        );
    }

    #[test]
    fn a_missing_trait_method_fix_omits_methods_with_defaults() {
        let module = check_source(
            "-",
            "trait Named { public function name() : String = \"anon\"\nfunction code() : Int }\nclass User { has Named }\n",
        );
        let fix = fix_for(&module, "E_TRAIT_METHOD");
        let stub = &fix.edits[0].text;
        assert!(stub.contains("function code() : Int"), "{stub}");
        assert!(!stub.contains("function name()"), "{stub}");
    }

    #[test]
    fn a_missing_match_case_becomes_an_empty_arm() {
        let module = check_source(
            "-",
            "enum Color { Red, Green }\nfunction f(c : Color) : Int = match c { Red => 1, }\n",
        );
        let fix = fix_for(&module, "E_MATCH_EXHAUSTIVE");
        assert!(fix.title.contains("Green"), "{}", fix.title);
        assert!(
            fix.edits[0].text.contains("Green => { },"),
            "{}",
            fix.edits[0].text
        );
    }

    #[test]
    fn a_missing_induction_case_names_the_hypothesis() {
        let module = check_source(
            "-",
            "theorem t(n : Nat) : n = n { induction n { Zero => rfl, } }\n",
        );
        let fix = fix_for(&module, "E_PROOF_CASES");
        assert!(fix.title.contains("Succ(k) ihk"), "{}", fix.title);
        assert!(
            fix.edits[0].text.contains("Succ(k) ihk => { },"),
            "{}",
            fix.edits[0].text
        );
    }

    #[test]
    fn an_inherited_class_method_satisfies_a_trait_without_a_stub() {
        let module = check_source(
            "-",
            "trait Shape { function area() : double }\nclass Base { public function area() : double = 1.0 }\nclass Child extends Base { has Shape }\n",
        );
        assert!(
            !module
                .checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E_TRAIT_METHOD"),
            "{:#?}",
            module.checked.diagnostics
        );
    }

    #[test]
    fn a_stub_keeps_the_traits_modifiers_and_effects() {
        let module = check_source(
            "-",
            "trait Signals {\n    function put(key : String) : Bool with Effects\n    mut function bump() : void\n    static function unit() : Int\n}\nclass C { has Signals }\n",
        );
        let fix = fix_for(&module, "E_TRAIT_METHOD");
        let text = &fix.edits[0].text;
        assert!(
            text.contains("public function put(key : String) : Bool with Effects"),
            "{text}"
        );
        assert!(text.contains("public mut function bump() : void"), "{text}");
        assert!(
            text.contains("public static function unit() : Int"),
            "{text}"
        );
    }

    #[test]
    fn several_missing_cases_offer_one_combined_action() {
        let module = check_source(
            "-",
            "enum Color { Red, Green, Blue }\nfunction f(c : Color) : Int = match c { Red => 1, }\n",
        );
        let fixes = &module
            .checked
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_MATCH_EXHAUSTIVE")
            .unwrap()
            .fixes;
        let combined = fixes
            .iter()
            .find(|fix| fix.title.contains("all 2"))
            .expect("a combined action");
        assert!(combined.edits[0].text.contains("Green => { }"));
        assert!(combined.edits[0].text.contains("Blue => { }"));
    }

    #[test]
    fn a_partly_covered_case_gets_an_arm_after_the_covered_ones() {
        let module = check_source(
            "-",
            "function f(o : Option[Nat]) : Nat = match o { Some(Zero) => 0, None => 1, }\n",
        );
        let fix = fix_for(&module, "E_MATCH_EXHAUSTIVE");
        assert!(
            fix.edits[0].text.contains("Some(value) => { }"),
            "{}",
            fix.edits[0].text
        );
    }

    #[test]
    fn an_open_error_fallback_becomes_a_wildcard() {
        let module = check_source(
            "-",
            "enum MyError extends Error { Bad(String) }\nfunction f(e : Error) : Int = match e { MyError.Bad(m) => 1, }\n",
        );
        let fix = fix_for(&module, "E_MATCH_EXHAUSTIVE");
        assert!(
            fix.edits[0].text.contains("_ => { }"),
            "{}",
            fix.edits[0].text
        );
    }

    #[test]
    fn a_missing_tactic_induction_branch_names_the_hypothesis() {
        let module = check_source(
            "-",
            "theorem t(n : Nat) : n = n by {\n    induction n {\n        Zero => { rfl }\n    }\n}\n",
        );
        let fix = fix_for(&module, "E_PROOF_CASES");
        assert!(fix.title.contains("Succ(k) ihk"), "{}", fix.title);
        assert!(
            fix.edits[0].text.contains("Succ(k) ihk => { }"),
            "{}",
            fix.edits[0].text
        );
    }

    #[test]
    fn a_missing_or_side_becomes_an_arm() {
        let module = check_source(
            "-",
            "theorem o(h : Zero = Zero or Zero = Zero) : Zero = Zero { match h { Left(p) => p, } }\n",
        );
        let fix = fix_for(&module, "E_PROOF_CASES");
        assert!(fix.title.contains("Right(q)"), "{}", fix.title);
        assert!(
            fix.edits[0].text.contains("Right(q) => { },"),
            "{}",
            fix.edits[0].text
        );
    }
}
