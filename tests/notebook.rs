//! `kestrel notebook`: cells in, marked blocks out. The marker is the record separator.

use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
};

const MARK: char = '\u{1e}';

struct Notebook {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    errors: BufReader<ChildStderr>,
    count: usize,
}

struct Reply {
    out: String,
    status: String,
}

impl Notebook {
    fn start() -> Self {
        let root = std::env::temp_dir();
        let mut child = Command::new(env!("CARGO_BIN_EXE_kestrel"))
            .args(["notebook", "--root"])
            .arg(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            errors: BufReader::new(child.stderr.take().unwrap()),
            child,
            count: 0,
        }
    }

    fn send(&mut self, kind: &str, code: &str) -> Reply {
        self.count += 1;
        write!(self.input, "{kind} {} {}\n{code}", self.count, code.len()).unwrap();
        self.input.flush().unwrap();
        let (out, status) = read_until_done(&mut self.output);
        let (_, _) = read_until_done(&mut self.errors);
        Reply { out, status }
    }

    fn cell(&mut self, code: &str) -> Reply {
        self.send("CELL", code)
    }
}

impl Drop for Notebook {
    fn drop(&mut self) {
        let _ = writeln!(self.input, "EXIT");
        let _ = self.child.wait();
    }
}

fn read_until_done(reader: &mut impl BufRead) -> (String, String) {
    let mut text = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            return (text, "dead".into());
        }
        if let Some(rest) = line.strip_prefix(&format!("{MARK}[done] ")) {
            return (
                text,
                rest.split_whitespace().last().unwrap_or("").to_owned(),
            );
        }
        text.push_str(&line);
    }
}

fn block(reply: &Reply, name: &str) -> Option<String> {
    let start = format!("{MARK}[{name}]\n");
    let begin = reply.out.find(&start)? + start.len();
    let end = reply.out[begin..].find(&format!("{MARK}[end]"))? + begin;
    Some(reply.out[begin..end].to_owned())
}

#[test]
fn only_the_last_expression_of_a_cell_is_displayed() {
    let mut notebook = Notebook::start();
    let reply = notebook.cell("let x := 40\nx\nx + 2");
    assert_eq!(reply.status, "ok");
    let shown = block(&reply, "result").unwrap();
    assert_eq!(shown, format!("{MARK}[text/plain]\n42\n"));
    assert_eq!(shown.matches("42").count(), 1);
}

#[test]
fn printed_text_stays_outside_the_result_block() {
    let mut notebook = Notebook::start();
    let reply = notebook.cell("print(\"hello\")\n\"value\"");
    assert!(reply.out.starts_with("hello\n"), "{:?}", reply.out);
    assert!(block(&reply, "result").unwrap().contains("value"));
}

#[test]
fn bindings_and_functions_persist_between_cells() {
    let mut notebook = Notebook::start();
    notebook.cell("let base := 10\nfunction bump(n : Int) : Int = n + base");
    let reply = notebook.cell("bump(5)");
    assert!(block(&reply, "result").unwrap().contains("15"));
}

#[test]
fn a_cell_that_does_not_compile_keeps_its_earlier_statements() {
    let mut notebook = Notebook::start();
    let reply = notebook.cell("let kept := 3\nlet broken := missing");
    assert_eq!(reply.status, "error");
    let message = block(&reply, "error").unwrap();
    assert!(message.contains("E_UNKNOWN_NAME"), "{message}");
    assert!(
        message.contains("<cell>:1:"),
        "line numbers count from the cell: {message}"
    );
    let reply = notebook.cell("kept");
    assert!(block(&reply, "result").unwrap().contains('3'));
}

#[test]
fn an_error_met_by_question_mark_ends_the_cell_and_keeps_the_session() {
    let mut notebook = Notebook::start();
    notebook.cell("function check(n : Int) : Int or Error { if n < 0 { return RuntimeError.Parse(\"negative\") }\nn }");
    notebook.cell("let before := 1");
    let reply = notebook.cell("let a := check(-1)?\nlet after := 2");
    assert_eq!(reply.status, "abort");
    assert!(block(&reply, "error").unwrap().contains("negative"));
    let reply = notebook.cell("before");
    assert!(block(&reply, "result").unwrap().contains('1'));
    // The name the aborted cell would have bound is free again.
    let reply = notebook.cell("let after := \"text\"\nafter");
    assert!(block(&reply, "result").unwrap().contains("text"));
}

#[test]
fn an_error_value_in_last_position_displays_as_an_error_block() {
    let mut notebook = Notebook::start();
    notebook.cell("function check(n : Int) : Int or Error { if n < 0 { return RuntimeError.Parse(\"negative\") }\nn }");
    let reply = notebook.cell("check(-4)");
    assert_eq!(reply.status, "ok");
    assert!(block(&reply, "error").unwrap().contains("negative"));
    let reply = notebook.cell("check(4)");
    assert!(block(&reply, "result").unwrap().contains('4'));
}

#[test]
fn types_with_an_html_method_send_an_html_part() {
    let mut notebook = Notebook::start();
    notebook.cell("class Badge { has Text\nnew() {}\npublic function text() : String = \"badge\"\npublic function html() : String = \"<b>badge</b>\" }");
    let reply = notebook.cell("Badge()");
    let shown = block(&reply, "result").unwrap();
    assert!(
        shown.contains(&format!("{MARK}[text/plain]\nbadge")),
        "{shown}"
    );
    assert!(
        shown.contains(&format!("{MARK}[text/html]\n<b>badge</b>")),
        "{shown}"
    );
}

#[test]
fn the_check_request_says_whether_a_cell_is_finished() {
    let mut notebook = Notebook::start();
    assert_eq!(notebook.send("CHECK", "let a := (1 +").status, "incomplete");
    assert_eq!(notebook.send("CHECK", "let a := 1").status, "complete");
}

#[test]
fn a_runtime_stop_ends_the_cell_and_keeps_the_session() {
    let mut notebook = Notebook::start();
    notebook.cell("let zero := 0");
    let reply = notebook.cell("10 / zero");
    assert_eq!(reply.status, "abort");
    let message = block(&reply, "error").unwrap();
    assert!(
        message.contains("<cell>:1:1: runtime error: Int division by zero"),
        "{message}"
    );
    assert!(message.contains("10 / zero"), "{message}");
    // The earlier cell's name still works, and the failed cell bound no name.
    let reply = notebook.cell("zero");
    assert!(block(&reply, "result").unwrap().contains('0'));
}

#[test]
fn an_index_outside_a_list_ends_the_cell_not_the_session() {
    let mut notebook = Notebook::start();
    notebook.cell("let values : List[Int] := [10, 20, 30]");
    let reply = notebook.cell("values.at(3)");
    assert_eq!(reply.status, "abort");
    let message = block(&reply, "error").unwrap();
    assert!(
        message.contains("<cell>:1:1: runtime error: List index out of bounds"),
        "{message}"
    );
    assert!(message.contains("values.at(3)"), "{message}");
    // Fixing the index works without redefining the list.
    let reply = notebook.cell("values.at(2)");
    assert!(block(&reply, "result").unwrap().contains("30"));
}

#[test]
fn figure_results_display_tex_as_html() {
    use std::process::Command;
    let tex = Command::new("latex").arg("--version").output().is_ok()
        && Command::new("dvisvgm").arg("--version").output().is_ok();
    if !tex {
        return;
    }
    let mut notebook = Notebook::start();
    let reply = notebook.cell(
        "import std.kesplot as plt\nlet fig := plt.Figure(style = plt.Style.science())\nfig.plot([0.0, 1.0], [0.0, 1.0])\nfig.title(\"$x^2$\")\nfig",
    );
    assert_eq!(reply.status, "ok", "{:?}", reply.out);
    let shown = block(&reply, "result").expect("result block");
    assert!(shown.contains("[text/html]"), "{shown}");
    assert!(shown.contains("<div"), "{shown}");
    assert!(
        shown.contains("<g transform="),
        "TeX is embedded in notebook HTML: {shown}"
    );
    assert!(!shown.contains("data-kestex"), "{shown}");
}
