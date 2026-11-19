//! A long-lived checker for editors.
//!
//! The editor opens one server per workspace folder and sends one JSON object
//! per line. Keeping the process alive is what makes checking interactive: the
//! standard library modules are checked once and then reused, so a keystroke
//! costs a parse and a check of the open file instead of a cold project check.
//!
//! Requests:
//!
//! ```text
//! {"id":1,"op":"check","path":"/work/hello.kes","text":"...","notebook":false}
//! {"id":2,"op":"goals","path":"/work/nat.kes","text":"...","offset":123}
//! {"id":3,"op":"actions","path":"/work/nat.kes","text":"...","start":100,"end":130}
//! {"id":4,"op":"ping"}
//! {"id":5,"op":"shutdown"}
//! ```
//!
//! A check answers with structured diagnostics, so the extension never has to
//! parse the compiler's human-readable output:
//!
//! ```text
//! {"id":1,"ok":true,"diagnostics":[{"line":2,"column":5,"endLine":2,
//!   "endColumn":9,"code":"E_TYPE","message":"...","hint":"..."}],"timeMs":4.2}
//! ```
//!
//! An actions request answers with the quick fixes that overlap its range. The
//! edits are UTF-16 offsets, ready for a VS Code `WorkspaceEdit`:
//!
//! ```text
//! {"id":3,"ok":true,"actions":[{"title":"Add the `Succ(k) ih` case",
//!   "code":"E_PROOF_CASES","kind":"quickfix",
//!   "edits":[{"start":80,"end":80,"text":"\n    Succ(k) ihk => { },\n"}]}]}
//! ```

use crate::{
    diagnostic::Diagnostic,
    modules::{ModuleCheck, check_source, check_text},
    proof::goals_at,
    source::{SourceFile, Span},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    time::Instant,
};

/// The protocol version. An editor refuses a server with a different version,
/// which turns an old compiler on `PATH` into a clear message instead of a
/// stream of malformed replies.
pub const PROTOCOL: u64 = 1;

/// Runs the editor server until standard input closes or a shutdown request
/// arrives. `root` is the package folder the editor resolves imports from: the
/// folder holding `kestrel.project`, which need not be the workspace folder.
pub fn run(root: PathBuf) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Cannot open {}: {error}", root.display()))?;
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| format!("Cannot read the editor request: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                reply(
                    &mut stdout,
                    &json!({"id": Value::Null, "ok": false, "error": format!("Malformed JSON: {error}")}),
                )?;
                continue;
            }
        };
        if request.get("op").and_then(Value::as_str) == Some("shutdown") {
            reply(
                &mut stdout,
                &json!({"id": request.get("id").cloned().unwrap_or(Value::Null), "ok": true}),
            )?;
            break;
        }
        let response = handle(&root, &request);
        reply(&mut stdout, &response)?;
    }
    Ok(())
}

fn reply(stdout: &mut impl Write, response: &Value) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|error| format!("Cannot encode a reply: {error}"))?;
    writeln!(stdout, "{encoded}").map_err(|error| format!("Cannot write a reply: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("Cannot flush a reply: {error}"))
}

fn handle(root: &Path, request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let op = request.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "hello" | "ping" => json!({
            "id": id,
            "ok": true,
            "protocol": PROTOCOL,
            "version": env!("CARGO_PKG_VERSION"),
        }),
        "check" => match check(root, request) {
            Ok(result) => {
                let mut response = json!({"id": id, "ok": true});
                merge(&mut response, result);
                response
            }
            Err(error) => json!({"id": id, "ok": false, "error": error}),
        },
        "goals" => match goals(root, request) {
            Ok(result) => {
                let mut response = json!({"id": id, "ok": true});
                merge(&mut response, result);
                response
            }
            Err(error) => json!({"id": id, "ok": false, "error": error}),
        },
        "actions" => match actions(root, request) {
            Ok(result) => {
                let mut response = json!({"id": id, "ok": true});
                merge(&mut response, result);
                response
            }
            Err(error) => json!({"id": id, "ok": false, "error": error}),
        },
        _ => json!({
            "id": id,
            "ok": false,
            "error": format!("Unknown operation `{op}`. This compiler speaks protocol {PROTOCOL}."),
        }),
    }
}

fn merge(target: &mut Value, extra: Value) {
    if let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// Checks one editor buffer. `path` keeps the file's identity so imports and
/// package rules match a check from the command line. `notebook` treats the
/// text as a session cell, where a top-level `?` is legal.
fn checked_buffer(root: &Path, request: &Value) -> Result<ModuleCheck, String> {
    let text = request
        .get("text")
        .and_then(Value::as_str)
        .ok_or("A request needs `text`.")?;
    let notebook = request
        .get("notebook")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if notebook {
        return Ok(check_source("<session>", text));
    }
    let path = request
        .get("path")
        .and_then(Value::as_str)
        .ok_or("A request needs `path`, or `notebook`: true.")?;
    let project = check_text(path, root, text.to_owned()).map_err(|error| error.to_string())?;
    // The edited buffer is named `-`; imported files keep their own names.
    // Only the edited buffer's diagnostics belong to the editor's document.
    project
        .modules
        .into_iter()
        .find(|module| module.parsed.source.name == "-")
        .ok_or("The checker did not return the edited buffer.".to_owned())
}

fn check(root: &Path, request: &Value) -> Result<Value, String> {
    let started = Instant::now();
    let module = checked_buffer(root, request)?;
    let diagnostics = diagnostics_json(&module);
    Ok(json!({
        "timeMs": started.elapsed().as_secs_f64() * 1000.0,
        "diagnostics": diagnostics,
    }))
}

/// The byte offset of a UTF-16 code-unit offset. VS Code positions count UTF-16
/// units; Kestrel spans count UTF-8 bytes.
fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (offset, ch) in text.char_indices() {
        if units >= utf16 {
            return offset;
        }
        units += ch.len_utf16();
    }
    text.len()
}

/// The UTF-16 code-unit offset of a byte offset, the inverse of `byte_offset`.
fn utf16_offset(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[..byte].encode_utf16().count()
}

/// Answers a request for the live goals at one cursor. The cursor is a UTF-16
/// offset into the checked text. A request outside a theorem returns no goals.
fn goals(root: &Path, request: &Value) -> Result<Value, String> {
    let module = checked_buffer(root, request)?;
    let text = &module.parsed.source.text;
    let offset = request
        .get("offset")
        .and_then(Value::as_u64)
        .ok_or("A goals request needs `offset`, in UTF-16 code units from the start of `text`.")?;
    let report = goals_at(
        &module.parsed,
        &module.checked,
        byte_offset(text, offset as usize),
    );
    let trust = crate::proof::trusted_theorems(&module.parsed, &module.checked)
        .into_iter()
        .find(|theorem| {
            theorem.span.start <= byte_offset(text, offset as usize)
                && byte_offset(text, offset as usize) <= theorem.span.end
        })
        .map(|theorem| theorem.reasons)
        .unwrap_or_default();
    let trusted = trust.iter().any(|reason| reason.starts_with("smt"));
    Ok(json!({
        "theorem": report.theorem,
        "goals": report.goals.iter().map(|goal| json!({
            "variables": goal.variables,
            "hypotheses": goal.hypotheses,
            "target": goal.target,
        })).collect::<Vec<_>>(),
        "cases": report.cases.iter().map(|case| json!({
            "label": case.label,
            "variables": case.goal.variables,
            "hypotheses": case.goal.hypotheses,
            "target": case.goal.target,
            "line": case.line,
            "column": case.column,
            "active": case.active,
        })).collect::<Vec<_>>(),
        "error": report.error,
        "complete": report.complete,
        "admitted": report.admitted,
        "trusted": trusted,
        "trustReasons": trust,
        "inTheorem": report.in_theorem,
    }))
}

/// Answers a request for quick fixes that overlap a UTF-16 range. The checker
/// attached each fix to the diagnostic that explains the gap, so this only
/// filters and serializes them.
fn actions(root: &Path, request: &Value) -> Result<Value, String> {
    let module = checked_buffer(root, request)?;
    let text = &module.parsed.source.text;
    let start = request.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
    let end = request
        .get("end")
        .and_then(Value::as_u64)
        .map(|end| end as usize)
        .unwrap_or_else(|| text.encode_utf16().count());
    let start = byte_offset(text, start);
    let end = byte_offset(text, end);
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let mut seen = BTreeSet::new();
    let mut found = vec![];
    for diagnostic in &module.checked.diagnostics {
        if !overlaps(diagnostic.span, start, end) {
            continue;
        }
        for fix in &diagnostic.fixes {
            if fix.edits.is_empty() {
                continue;
            }
            let edits = fix
                .edits
                .iter()
                .map(|edit| {
                    json!({
                        "start": utf16_offset(text, edit.span.start),
                        "end": utf16_offset(text, edit.span.end),
                        "text": edit.text,
                    })
                })
                .collect::<Vec<_>>();
            let key = format!("{}\u{0}{edits:?}", fix.title);
            if !seen.insert(key) {
                continue;
            }
            found.push(json!({
                "title": fix.title,
                "code": diagnostic.code,
                "kind": "quickfix",
                "edits": edits,
            }));
        }
    }
    Ok(json!({"actions": found}))
}

fn overlaps(span: Span, start: usize, end: usize) -> bool {
    if start == end {
        span.start <= start && start <= span.end
    } else {
        span.start < end && start < span.end
    }
}

fn diagnostics_json(module: &ModuleCheck) -> Vec<Value> {
    module
        .checked
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic_json(diagnostic, &module.parsed.source))
        .collect()
}

fn diagnostic_json(diagnostic: &Diagnostic, source: &SourceFile) -> Value {
    let (line, column) = source.position(diagnostic.span.start);
    // A caret is one character wide when the parser points between two tokens.
    let end = diagnostic.span.end.max(diagnostic.span.start + 1);
    let (end_line, end_column) = source.position(end);
    json!({
        "line": line,
        "column": column,
        "endLine": end_line,
        "endColumn": end_column,
        "severity": match diagnostic.severity {
            crate::diagnostic::Severity::Error => "error",
            crate::diagnostic::Severity::Warning => "warning",
        },
        "code": diagnostic.code,
        "message": diagnostic.message,
        "hint": diagnostic.hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diagnostic_carries_one_based_lines_and_columns() {
        let module = check_source("-", "let a : String = 1\n");
        let found = diagnostics_json(&module);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["line"], 1);
        assert_eq!(found[0]["column"], 18);
        assert_eq!(found[0]["code"], "E_TYPE");
        assert!(found[0]["hint"].is_string());
    }

    #[test]
    fn the_hello_request_reports_the_protocol() {
        assert_eq!(
            handle(&PathBuf::from("."), &json!({"id": 1, "op": "hello"}))["protocol"],
            1
        );
    }

    #[test]
    fn an_action_request_returns_the_generated_proof_case() {
        let text = "theorem t(n : Nat) : n = n { induction n { Zero => rfl, } }\n";
        let response = handle(
            &PathBuf::from("."),
            &json!({
                "id": 1,
                "op": "actions",
                "notebook": true,
                "text": text,
                "start": 0,
                "end": text.len(),
            }),
        );
        assert_eq!(response["ok"], true);
        let actions = response["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0]["code"], "E_PROOF_CASES");
        assert!(
            actions[0]["edits"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Succ(k) ihk => { }")
        );
    }

    #[test]
    fn actions_outside_the_requested_range_stay_hidden() {
        let text = "trait Shape { function area() : double }\nclass C { has Shape }\n";
        let response = handle(
            &PathBuf::from("."),
            &json!({
                "id": 1,
                "op": "actions",
                "notebook": true,
                "text": text,
                "start": 0,
                "end": 0,
            }),
        );
        assert_eq!(response["ok"], true);
        assert_eq!(response["actions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn an_action_edit_counts_utf16_units_like_the_editor() {
        // The emoji is one UTF-16 pair before the class; the edit offset must
        // count that pair as two units, as VS Code does.
        let text = "let label := \"😀\"\ntrait Shape { function area() : double }\nclass C { has Shape }\n";
        let start = text[..text.find("class").unwrap()].encode_utf16().count();
        let end = start + "class C { has Shape }".encode_utf16().count();
        let response = handle(
            &PathBuf::from("."),
            &json!({
                "id": 1,
                "op": "actions",
                "notebook": true,
                "text": text,
                "start": start,
                "end": end,
            }),
        );
        let actions = response["actions"].as_array().unwrap();
        assert!(!actions.is_empty());
        let edit_start = actions[0]["edits"][0]["start"].as_u64().unwrap() as usize;
        assert_eq!(
            edit_start,
            text[..text.rfind('}').unwrap()].encode_utf16().count()
        );
    }
}
