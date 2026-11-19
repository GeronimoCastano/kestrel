//! Brackets on user classes through the prelude's Indexable and MutableIndexable traits.
//! Reads lower to at(key), writes to setAt(key, value); built-in brackets keep their rules.
use kestrel::{
    modules::{ProjectCheck, check_source},
    native::TemporaryDirectory,
};
use std::process::{Command, Output};

const GRID: &str = r####"class Grid {
    has MutableIndexable[(Int, Int), double]

    private var cells : Array[double]
    private final columns : Int

    new(rows : Int, columns : Int) {
        this.columns = columns
        this.cells = Array(rows * columns, 0.0)
    }

    public function at(key : (Int, Int)) : double = this.cells[this.columns * key[0] + key[1]]

    public mut function setAt(key : (Int, Int), value : double) : void {
        this.cells[this.columns * key[0] + key[1]] = value
    }
}
"####;

const READ_ONLY_GRID: &str = r####"class Grid {
    has Indexable[(Int, Int), double]

    private var cells : Array[double]
    private final columns : Int

    new(rows : Int, columns : Int) {
        this.columns = columns
        this.cells = Array(rows * columns, 0.0)
    }

    public function at(key : (Int, Int)) : double = this.cells[this.columns * key[0] + key[1]]
}
"####;

fn check(source: &str) -> ProjectCheck {
    ProjectCheck {
        modules: vec![check_source("indexing.kes", source)],
    }
}
fn good(source: &str) {
    let result = check(source);
    assert!(result.is_ok(), "{source}\n{}", result.render_diagnostics());
}
fn refuses(source: &str, code: &str) {
    let result = check(source);
    let text = result.render_diagnostics();
    assert!(text.contains(code), "Expected {code}: {source}\n{text}");
}
/// Runs a program through `kestrel run`, which loads the prelude and standard modules.
fn run(source: &str) -> Output {
    let directory = TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    std::fs::write(directory.path().join("main.kes"), source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_kestrel"))
        .arg("run")
        .arg("main.kes")
        .current_dir(directory.path())
        .output()
        .unwrap()
}
fn stdout(source: &str) -> String {
    let output = run(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn a_grid_reads_and_writes_with_a_tuple_key() {
    let output = stdout(&format!(
        "{GRID}let mut grid := Grid(2, 3)\ngrid[(0, 2)] = 5.5\ngrid[(1, 0)] = 1.5\nprint(grid[(0, 2)])\nprint(grid[(1, 0)])\nprint(grid[(0, 2)] + grid[(1, 0)])\n"
    ));
    assert_eq!(output, "5.5\n1.5\n7.0\n");
}

#[test]
fn a_borrowed_parameter_reads_and_only_a_mut_parameter_writes() {
    good(&format!(
        "{GRID}function read(grid : Grid) : double = grid[(0, 1)]\nfunction write(mut grid : Grid) : void {{ grid[(0, 1)] = 2.0 }}\nlet mut grid := Grid(2, 3)\nprint(read(grid))\nwrite(grid)"
    ));
    refuses(
        &format!("{GRID}function write(grid : Grid) : void {{ grid[(0, 1)] = 2.0 }}"),
        "E_MUT_PARAMETER",
    );
}

#[test]
fn a_write_needs_the_mutable_trait() {
    refuses(
        &format!("{READ_ONLY_GRID}let mut grid := Grid(2, 3)\ngrid[(0, 1)] = 2.0"),
        "E_INDEX_WRITE",
    );
    let text = check(&format!(
        "{READ_ONLY_GRID}let mut grid := Grid(2, 3)\ngrid[(0, 1)] = 2.0"
    ))
    .render_diagnostics();
    assert!(text.contains("MutableIndexable"), "{text}");
}

#[test]
fn a_wrong_key_type_names_the_trait_and_the_key() {
    let source = format!("{GRID}let grid := Grid(2, 3)\nprint(grid[1])");
    refuses(&source, "E_INDEX_KEY");
    let text = check(&source).render_diagnostics();
    assert!(text.contains("Indexable"), "{text}");
    assert!(text.contains("(Int, Int)"), "{text}");
    assert!(text.contains("Int"), "{text}");
}

#[test]
fn a_class_without_the_trait_names_it() {
    let source = "class Plain {\n    public final n : Int\n    new(n : Int) { this.n = n }\n}\nlet plain := Plain(2)\nprint(plain[0])";
    refuses(source, "E_INDEX_TRAIT");
    let text = check(source).render_diagnostics();
    assert!(text.contains("Indexable"), "{text}");
}

#[test]
fn direct_at_calls_keep_working_beside_brackets() {
    let output = stdout(&format!(
        "{GRID}let mut grid := Grid(2, 3)\ngrid[(1, 2)] = 4.5\ngrid.setAt((0, 0), 3.0)\nprint(grid[(1, 2)])\nprint(grid.at((1, 2)))\nprint(grid.at((0, 0)))\n"
    ));
    assert_eq!(output, "4.5\n4.5\n3.0\n");
}

#[test]
fn a_read_may_borrow_a_class_item() {
    let output = stdout(
        r####"class Cell {
    public var value : double
    new(value : double) { this.value = value }
}

class Row {
    has Indexable[Int, Cell]

    private var cells : Array[Cell]

    new() {
        this.cells = Array.generate(3, i => Cell(i.toDouble()))
    }

    public function at(i : Int) : Cell = this.cells[i]
}

function first(row : Row) : double = row[0].value

let row := Row()
print(first(row))
print(row[2].value)
row[2].value = 42.0
print(row[2].value)
"####,
    );
    assert_eq!(output, "0.0\n2.0\n42.0\n");
}

#[test]
fn built_in_array_brackets_are_unchanged() {
    assert_eq!(
        stdout("let mut xs := Array(3, 1)\nxs[0] = 5\nprint(xs[0])\nprint(xs[1])\n"),
        "5\n1\n"
    );
    refuses(
        "function f(xs : Array[Int]) : void { xs[0] = 1 }",
        "E_MUT_PARAMETER",
    );
    refuses("let xs := [1, 2]\nlet v := xs[0]", "E_INDEX_TYPE");
}

#[test]
fn built_in_string_brackets_are_unchanged() {
    assert_eq!(
        stdout("let text := \"héllo\"\nprint(text[1])\nprint(text[1:3])\n"),
        "é\nél\n"
    );
    refuses("let text := \"abc\"\nprint(text[true])", "E_TYPE");
}

#[test]
fn built_in_tensor_brackets_are_unchanged() {
    assert_eq!(
        stdout(
            "let mut t := Tensor([1.0, 2.0, 3.0, 4.0], shape = (2, 2))\nprint(t[1, 0])\nt[0, 1] = 9.0\nprint(t[0, 1])\n"
        ),
        "3.0\n9.0\n"
    );
}

#[test]
fn a_type_name_in_brackets_still_builds_a_constructor() {
    good("let channel := Channel[Int]()\nprint(1)");
}

#[test]
fn a_record_can_implement_the_trait() {
    let output = stdout(
        r####"record Price(amount : double) {
    has Indexable[Int, double]

    public function at(key : Int) : double = this.amount + key.toDouble()
}

let price := Price(10.0)
print(price[2])
print(price.at(2))
"####,
    );
    assert_eq!(output, "12.0\n12.0\n");
}

#[test]
fn the_example_checks_and_runs() {
    let source = include_str!("../testdata/language/indexing.kes");
    good(source);
    assert_eq!(stdout(source), "2.0\n4.0\n12.0\n99.0\n");
}
