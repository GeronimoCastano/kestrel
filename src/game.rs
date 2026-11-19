//! A small file-based game server for Kestrel.
//!
//! A game is a directory holding `game.json`, a shared `theory.kes` prelude and
//! one level file per theorem. Level files are ordinary Kestrel files whose
//! model proof sits between `// [[proof]]` and `// [[/proof]]`. A submission
//! replaces that region and is checked by the same kernel as the compiler, so
//! the browser never decides whether a proof is accepted.
//!
//! The browser page talks to this module over localhost HTTP. The server has no
//! dependencies beyond the ones the compiler already uses.
use crate::{
    diagnostic::{Diagnostic, Severity},
    modules::check_source,
    proof::{ProofGoal, goals_at},
    source::{SourceFile, Span},
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::Arc,
    thread,
};

/// One game directory, described by its `game.json` manifest.
#[derive(Clone, Debug)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub description: String,
    pub worlds: Vec<World>,
    pub dir: PathBuf,
}

/// One world in a game's manifest.
#[derive(Clone, Debug)]
pub struct World {
    pub id: String,
    pub title: String,
    pub intro: String,
    pub levels: Vec<Level>,
}

/// One level: its manifest metadata plus the checked source and hole position.
#[derive(Clone, Debug)]
pub struct Level {
    pub id: String,
    pub title: String,
    pub blurb: String,
    pub hint: String,
    pub statement: String,
    /// The level file's path, relative to the game directory.
    pub file: String,
    /// `theory.kes` followed by the level file. The proof markers stay in the
    /// text, so a submission only has to replace `proof_start..proof_end`.
    pub source: String,
    /// Optional display starter from the manifest; empty when absent.
    pub starter: String,
    /// The committed model proof between the markers.
    pub solution: String,
    /// Length of the `theory.kes` prefix inside `source`.
    theory_len: usize,
    proof_start: usize,
    proof_end: usize,
}

/// The result of checking one submitted proof.
#[derive(Clone, Debug, Default)]
pub struct SubmissionReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub goals: Vec<ProofGoal>,
    pub theorem: String,
    pub complete: bool,
    /// The checked theorem depends on an admitted proof, such as a theory
    /// axiom or an imported theorem proved with `sorry`.
    pub admitted: bool,
    /// The submitted script itself calls `sorry`. A game solution may rely on
    /// the game's declared axioms but not on the player's own `sorry`.
    pub sorry: bool,
    pub error: Option<String>,
}

/// The markers that delimit the one proof hole in a level file.
const PROOF_OPEN: &str = "[[proof]]";
const PROOF_CLOSE: &str = "[[/proof]]";

/// Reads and validates one game directory. Every manifest field that points at
/// a file is resolved now, so a game either loads completely or not at all.
pub fn load_game(dir: &Path) -> Result<Game, String> {
    let manifest_path = dir.join("game.json");
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "{}: cannot read the manifest: {error}",
            manifest_path.display()
        )
    })?;
    let manifest: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{}: invalid JSON: {error}", manifest_path.display()))?;
    let manifest = manifest.as_object().ok_or_else(|| {
        format!(
            "{}: the manifest must be a JSON object",
            manifest_path.display()
        )
    })?;

    let id = string_field(manifest, "id", "game.json")?;
    if id.is_empty() {
        return Err("game.json: `id` is empty".into());
    }
    let title = string_field(manifest, "title", "game.json")?;
    let description = optional_string_field(manifest, "description", "game.json")?;
    let worlds = array_field(manifest, "worlds", "game.json")?;

    let theory_path = dir.join("theory.kes");
    let theory = fs::read_to_string(&theory_path).map_err(|error| {
        format!(
            "{}: cannot read the shared theory: {error}",
            theory_path.display()
        )
    })?;

    let mut world_ids = BTreeSet::new();
    let mut level_ids = BTreeSet::new();
    let mut parsed_worlds = Vec::with_capacity(worlds.len());
    for (world_index, world) in worlds.iter().enumerate() {
        let location = format!("game.json: worlds[{world_index}]");
        let world = object(world, &location)?;
        let world_id = string_field(world, "id", &location)?;
        if world_id.is_empty() {
            return Err(format!("{location}: `id` is empty"));
        }
        if !world_ids.insert(world_id.clone()) {
            return Err(format!("game.json: duplicate world id `{world_id}`"));
        }
        let world_title = string_field(world, "title", &location)?;
        let intro = optional_string_field(world, "intro", &location)?;
        let levels = array_field(world, "levels", &location)?;
        let mut parsed_levels = Vec::with_capacity(levels.len());
        for (level_index, level) in levels.iter().enumerate() {
            let location = format!("game.json: world `{world_id}` levels[{level_index}]");
            let level = object(level, &location)?;
            let level_id = string_field(level, "id", &location)?;
            if level_id.is_empty() {
                return Err(format!("{location}: `id` is empty"));
            }
            if !level_ids.insert(level_id.clone()) {
                return Err(format!("game.json: duplicate level id `{level_id}`"));
            }
            let level_title = string_field(level, "title", &location)?;
            let blurb = optional_string_field(level, "blurb", &location)?;
            let hint = optional_string_field(level, "hint", &location)?;
            let statement = string_field(level, "statement", &location)?;
            let starter = optional_string_field(level, "starter", &location)?;
            let file = string_field(level, "file", &location)?;
            check_relative_path(&file).map_err(|error| format!("{location}: {error}"))?;
            let path = dir.join(&file);
            let raw = fs::read_to_string(&path)
                .map_err(|error| format!("{location}: cannot read `{file}`: {error}"))?;
            // A level file may carry `import ../theory.kes` so it also checks on
            // its own. The game prepends the theory, so leading imports would be
            // redundant (and out of order) in the combined source.
            let level_source = strip_leading_imports(&raw);
            let (local_start, local_end, solution) = find_hole(&level_source, &file)?;

            let mut source = String::with_capacity(theory.len() + level_source.len() + 1);
            source.push_str(&theory);
            if !theory.is_empty() && !theory.ends_with('\n') {
                source.push('\n');
            }
            let shift = source.len();
            source.push_str(&level_source);
            parsed_levels.push(Level {
                id: level_id,
                title: level_title,
                blurb,
                hint,
                statement,
                file,
                source,
                starter,
                solution,
                theory_len: shift,
                proof_start: shift + local_start,
                proof_end: shift + local_end,
            });
        }
        parsed_worlds.push(World {
            id: world_id,
            title: world_title,
            intro,
            levels: parsed_levels,
        });
    }

    Ok(Game {
        id,
        title,
        description,
        worlds: parsed_worlds,
        dir: dir.to_path_buf(),
    })
}

/// Discovers the immediate subdirectories of `root` that hold a `game.json`,
/// sorts them by game id and skips directories that do not load. The server
/// reports every skipped directory on stderr.
pub fn load_games(root: &Path) -> Vec<Game> {
    let mut games = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return games;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.join("game.json").is_file() {
            continue;
        }
        match load_game(&path) {
            Ok(game) => games.push(game),
            Err(error) => eprintln!("kestrel game: {error}"),
        }
    }
    games.sort_by(|left, right| left.id.cmp(&right.id));
    games.dedup_by(|left, right| left.id == right.id);
    games
}

/// Splices `script` into the level's marked hole and checks the result with the
/// ordinary module checker. Goals are reported at the end of the script.
pub fn check_submission(level: &Level, script: &str) -> SubmissionReport {
    check_submission_at(level, script, None)
}

/// Like [`check_submission`], but reports the goals at a cursor measured in
/// UTF-16 code units from the start of the script. The browser sends
/// `textarea.selectionStart`, which uses the same unit.
pub fn check_submission_at(level: &Level, script: &str, cursor: Option<usize>) -> SubmissionReport {
    let mut source = String::with_capacity(level.source.len() + script.len());
    source.push_str(&level.source[..level.proof_start]);
    source.push_str(script);
    source.push_str(&level.source[level.proof_end..]);
    let module = check_source(level.file.clone(), source);
    let script_end = level.proof_start + script.len();

    let mut errors = Vec::new();
    let mut failed = false;
    for diagnostic in &module.checked.diagnostics {
        if diagnostic.severity != Severity::Error {
            continue;
        }
        // The remaining goal is already part of the report. Keeping this
        // summary diagnostic would print the same fact twice and point at the
        // theorem header instead of the editor.
        if diagnostic.code == "E_PROOF_INCOMPLETE" {
            continue;
        }
        failed = true;
        errors.push(render_diagnostic(diagnostic, level, script));
    }

    let offset = match cursor {
        Some(cursor) => level.proof_start + byte_offset(script, cursor),
        None => script_end,
    };
    let report = goals_at(&module.parsed, &module.checked, offset);
    let sorry = script_uses_sorry(script);
    let ok = !failed && report.complete && !sorry && report.error.is_none();
    SubmissionReport {
        ok,
        errors,
        goals: report.goals,
        theorem: report.theorem,
        complete: report.complete,
        admitted: report.admitted,
        sorry,
        error: report.error,
    }
}

/// Whether a submitted script calls `sorry` itself. Theory axioms are trusted
/// game content and set the kernel's admitted flag, so the server looks for the
/// player's own `sorry` token instead: words only, outside `//` comments and
/// string literals.
fn script_uses_sorry(script: &str) -> bool {
    let bytes = script.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            quote @ (b'"' | b'\'') => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == quote {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                if &script[start..index] == "sorry" {
                    return true;
                }
            }
            _ => index += 1,
        }
    }
    false
}

/// Renders a diagnostic against the submitted script when its span is inside
/// the hole, and against the level file or theory otherwise. Player mistakes
/// therefore keep line numbers relative to the editor.
fn render_diagnostic(diagnostic: &Diagnostic, level: &Level, script: &str) -> String {
    if diagnostic.span.start >= level.proof_start
        && diagnostic.span.end <= level.proof_start + script.len()
    {
        let mut local = diagnostic.clone();
        local.span = Span::new(
            diagnostic.span.start - level.proof_start,
            diagnostic.span.end - level.proof_start,
        );
        return local.render(&SourceFile::new("proof", script));
    }
    if diagnostic.span.start >= level.theory_len {
        let mut local = diagnostic.clone();
        local.span = Span::new(
            diagnostic.span.start - level.theory_len,
            diagnostic.span.end.saturating_sub(level.theory_len),
        );
        return local.render(&SourceFile::new(
            level.file.clone(),
            &level.source[level.theory_len..],
        ));
    }
    diagnostic.render(&SourceFile::new(
        "theory.kes",
        &level.source[..level.theory_len],
    ))
}

/// Maps a cursor in UTF-16 code units to a UTF-8 byte offset.
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

/// Removes import declarations from the leading comment/blank block of a level
/// file while keeping its line count stable.
fn strip_leading_imports(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut before_code = true;
    for line in source.split_inclusive('\n') {
        if before_code {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                result.push_str(line);
                continue;
            }
            if trimmed == "import" || trimmed.starts_with("import ") {
                // Keep the newline so diagnostics in the level line up with the
                // file the author sees.
                result.push('\n');
                continue;
            }
            before_code = false;
        }
        result.push_str(line);
    }
    result
}

/// Finds the single proof hole and returns the byte range between its marker
/// lines plus the committed solution text.
fn find_hole(source: &str, file: &str) -> Result<(usize, usize, String), String> {
    let mut open: Option<usize> = None;
    let mut open_line = 0;
    let mut hole: Option<(usize, usize)> = None;
    let mut offset = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        let marker = trimmed.strip_prefix("//").map(str::trim);
        match marker {
            Some(open_marker) if open_marker == PROOF_OPEN => {
                if open.is_some() {
                    return Err(format!(
                        "{file}:{number}: nested `// [[proof]]` marker; a level has exactly one hole"
                    ));
                }
                if hole.is_some() {
                    return Err(format!(
                        "{file}:{number}: second `// [[proof]]` marker; a level has exactly one hole"
                    ));
                }
                open = Some(offset + line.len());
                open_line = number;
            }
            Some(close_marker) if close_marker == PROOF_CLOSE => {
                let Some(start) = open.take() else {
                    return Err(format!(
                        "{file}:{number}: `// [[/proof]]` without a matching `// [[proof]]`"
                    ));
                };
                hole = Some((start, offset));
            }
            _ if trimmed.contains("[[proof") || trimmed.contains("[[/proof") => {
                return Err(format!(
                    "{file}:{number}: malformed proof marker `{trimmed}`; put `// [[proof]]` and `// [[/proof]]` on their own lines"
                ));
            }
            _ => {}
        }
        offset += line.len();
    }
    if open.is_some() {
        return Err(format!(
            "{file}:{open_line}: unclosed `// [[proof]]` marker; add `// [[/proof]]` after the model proof"
        ));
    }
    let Some((start, end)) = hole else {
        return Err(format!(
            "{file}: no proof hole; add `// [[proof]]` and `// [[/proof]]` around the model proof"
        ));
    };
    Ok((start, end, source[start..end].to_string()))
}

fn object<'a>(value: &'a Value, location: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{location} must be a JSON object"))
}

fn string_field(object: &Map<String, Value>, key: &str, location: &str) -> Result<String, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format!("{location} is missing `{key}`"))?;
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{location}.{key} must be a string"))
}

fn optional_string_field(
    object: &Map<String, Value>,
    key: &str,
    location: &str,
) -> Result<String, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(format!("{location}.{key} must be a string")),
    }
}

fn array_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    location: &str,
) -> Result<&'a Vec<Value>, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{location} is missing `{key}`"))?
        .as_array()
        .ok_or_else(|| format!("{location}.{key} must be an array"))
}

/// A level path must stay inside the game directory. Absolute paths and `..`
/// components are refused before anything is read.
fn check_relative_path(file: &str) -> Result<(), String> {
    if file.is_empty() {
        return Err("`file` is empty".into());
    }
    let path = Path::new(file);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!(
            "`file` must be a relative path inside the game directory, not `{file}`"
        ));
    }
    Ok(())
}

fn catalog_json(games: &[Game]) -> Value {
    Value::Array(
        games
            .iter()
            .map(|game| {
                let levels = game
                    .worlds
                    .iter()
                    .map(|world| world.levels.len())
                    .sum::<usize>();
                json!({
                    "id": game.id,
                    "title": game.title,
                    "description": game.description,
                    "worlds": game.worlds.len(),
                    "levels": levels,
                })
            })
            .collect(),
    )
}

/// The public game payload. The theory, the model proof and the full combined
/// source stay on the server; the browser only sees metadata and display text.
fn game_json(game: &Game) -> Value {
    json!({
        "id": game.id,
        "title": game.title,
        "description": game.description,
        "worlds": game.worlds.iter().map(|world| json!({
            "id": world.id,
            "title": world.title,
            "intro": world.intro,
            "levels": world.levels.iter().enumerate().map(|(index, level)| json!({
                "index": index + 1,
                "id": level.id,
                "title": level.title,
                "blurb": level.blurb,
                "hint": level.hint,
                "statement": level.statement,
                "starter": level.starter,
                "file": level.file,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn report_json(report: &SubmissionReport) -> Value {
    json!({
        "ok": report.ok,
        "errors": report.errors,
        "goals": report.goals.iter().map(|goal| json!({
            "variables": goal.variables,
            "hypotheses": goal.hypotheses,
            "target": goal.target,
        })).collect::<Vec<_>>(),
        "theorem": report.theorem,
        "complete": report.complete,
        "admitted": report.admitted,
        "sorry": report.sorry,
        "error": report.error,
    })
}

fn find_game<'a>(games: &'a [Game], id: &str) -> Option<&'a Game> {
    games.iter().find(|game| game.id == id)
}

fn find_level<'a>(game: &'a Game, id: &str) -> Option<&'a Level> {
    game.worlds
        .iter()
        .flat_map(|world| world.levels.iter())
        .find(|level| level.id == id)
}

fn response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    )
}

fn json_response(status: &str, value: &Value) -> String {
    response(
        status,
        "application/json",
        &serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
    )
}

fn error_json(status: &str, message: impl Into<String>) -> String {
    json_response(status, &json!({"ok": false, "error": message.into()}))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn handle(mut stream: TcpStream, games: &[Game]) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    break position + 4;
                }
                if buffer.len() > 1 << 20 {
                    return;
                }
            }
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.lines();
    let Some(request) = lines.next() else { return };
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let mut length = 0;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }
    if length > 1 << 20 {
        let reply = error_json("413 Payload Too Large", "The request body is too large.");
        let _ = stream.write_all(reply.as_bytes());
        return;
    }
    while buffer.len() < header_end + length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
        }
    }
    let body_end = (header_end + length).min(buffer.len());
    let body = String::from_utf8_lossy(&buffer[header_end.min(body_end)..body_end]).into_owned();

    let reply = route(method, path, &body, games);
    let _ = stream.write_all(reply.as_bytes());
}

fn route(method: &str, path: &str, body: &str, games: &[Game]) -> String {
    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") | ("GET", "/game.html") => response(
            "200 OK",
            "text/html; charset=utf-8",
            include_str!("game.html"),
        ),
        ("GET", "/api/games") => json_response("200 OK", &catalog_json(games)),
        ("GET", "/api/health") => json_response("200 OK", &json!({"ok": true})),
        ("GET", "/favicon.ico") => response("204 No Content", "text/plain", ""),
        ("POST", "/api/check") => {
            let request: Value = match serde_json::from_str(body) {
                Ok(request) => request,
                Err(error) => {
                    return error_json("400 Bad Request", format!("Malformed JSON: {error}"));
                }
            };
            let (Some(game_id), Some(level_id), Some(proof)) = (
                request.get("game").and_then(Value::as_str),
                request.get("level").and_then(Value::as_str),
                request.get("proof").and_then(Value::as_str),
            ) else {
                return error_json(
                    "400 Bad Request",
                    "A check needs `game`, `level` and `proof` strings.",
                );
            };
            let Some(game) = find_game(games, game_id) else {
                return error_json("404 Not Found", format!("There is no game `{game_id}`."));
            };
            let Some(level) = find_level(game, level_id) else {
                return error_json(
                    "404 Not Found",
                    format!("There is no level `{level_id}` in `{game_id}`."),
                );
            };
            let cursor = request
                .get("offset")
                .and_then(Value::as_u64)
                .map(|offset| offset as usize);
            let report = check_submission_at(level, proof, cursor);
            json_response("200 OK", &report_json(&report))
        }
        ("GET", _) if path.starts_with("/api/game/") => {
            let raw = &path["/api/game/".len()..];
            if raw.contains('/') {
                return error_json("404 Not Found", "Not found.");
            }
            let Some(id) = percent_decode(raw) else {
                return error_json("400 Bad Request", "The game id is not valid UTF-8.");
            };
            match find_game(games, &id) {
                Some(game) => json_response("200 OK", &game_json(game)),
                None => error_json("404 Not Found", format!("There is no game `{id}`.")),
            }
        }
        ("GET", _) if path.starts_with("/api/solution/") => {
            let raw = &path["/api/solution/".len()..];
            let mut parts = raw.split('/');
            let (Some(game_part), Some(level_part), None) =
                (parts.next(), parts.next(), parts.next())
            else {
                return error_json("404 Not Found", "Not found.");
            };
            let (Some(game_id), Some(level_id)) =
                (percent_decode(game_part), percent_decode(level_part))
            else {
                return error_json("400 Bad Request", "The solution path is not valid UTF-8.");
            };
            let Some(game) = find_game(games, &game_id) else {
                return error_json("404 Not Found", format!("There is no game `{game_id}`."));
            };
            let Some(level) = find_level(game, &level_id) else {
                return error_json(
                    "404 Not Found",
                    format!("There is no level `{level_id}` in `{game_id}`."),
                );
            };
            json_response(
                "200 OK",
                &json!({
                    "game": game.id,
                    "level": level.id,
                    "solution": level.solution,
                }),
            )
        }
        _ => error_json("404 Not Found", "Not found."),
    }
}

/// Serves the games under `root` on `127.0.0.1:port` until the process stops.
pub fn run(port: u16, root: &Path) -> Result<(), String> {
    let games = load_games(root);
    let levels = games
        .iter()
        .map(|game| {
            game.worlds
                .iter()
                .map(|world| world.levels.len())
                .sum::<usize>()
        })
        .sum::<usize>();
    if games.is_empty() {
        eprintln!(
            "kestrel game: no games found in {}; add a directory with game.json",
            root.display()
        );
    }
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("Cannot listen on port {port}: {error}"))?;
    println!(
        "Kestrel game: {} game(s), {} level(s) from {}",
        games.len(),
        levels,
        root.display()
    );
    println!("Kestrel game: http://127.0.0.1:{port}");
    let games = Arc::new(games);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let games = Arc::clone(&games);
                // The kernel and the parser recurse over the source. The default
                // spawn stack is too small for a large theory such as the number
                // theory game, so give each request a generous stack.
                let spawned = thread::Builder::new()
                    .stack_size(32 * 1024 * 1024)
                    .spawn(move || handle(stream, &games));
                if let Err(error) = spawned {
                    eprintln!("kestrel game: {error}");
                }
            }
            Err(error) => eprintln!("kestrel game: {error}"),
        }
    }
    Ok(())
}
