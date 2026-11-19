//! kesdas, the pandas-style library: every case in tests/kesdas/program.kes prints what
//! pandas prints for the same operation. tests/kesdas/expected.txt holds pandas' output;
//! regenerate both files with `python3 tools/compare_kesdas.py --emit-test`.

use kestrel::{core, modules::check_path, native};
use std::{fs, path::Path, process::Command};

fn run_program(source: &str) -> String {
    let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
    let file = dir.path().join("program.kes");
    fs::write(&file, source).unwrap();
    let project = check_path(&file, dir.path()).unwrap();
    assert!(project.is_ok(), "{}", project.render_diagnostics());
    let program = core::lower(&project).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let binary = dir.path().join("program");
    native::build(&program, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn on_big_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn every_case_prints_what_pandas_prints() {
    on_big_stack(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kesdas");
        let source = fs::read_to_string(root.join("program.kes")).unwrap();
        let expected = fs::read_to_string(root.join("expected.txt")).unwrap();
        let got = run_program(&source);
        // Report the first differing case, not one giant diff.
        let wanted = expected.split("=== ").collect::<Vec<_>>();
        let found = got.split("=== ").collect::<Vec<_>>();
        assert_eq!(wanted.len(), found.len(), "different number of cases");
        for (want, have) in wanted.iter().zip(&found) {
            assert_eq!(want.trim_end(), have.trim_end(), "case differs from pandas");
        }
    });
}

#[test]
fn the_example_program_runs() {
    on_big_stack(|| {
        let source = include_str!("../testdata/language/kesdas.kes");
        let got = run_program(source);
        assert!(got.contains("north     285    7.5    712.5"), "{got}");
        assert!(
            got.contains("3    east     60    4.0    240.0     NaN"),
            "{got}"
        );
    });
}

#[test]
fn csv_text_round_trips_with_quotes_and_missing_entries() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{DataFrame}
let text := "name,age,score\nAna,30,1.5\n\"Lee, Jo\",25,\nBo,,3.25\n"
let frame := DataFrame.parseCsv(text)
print(frame)
print(frame.dtypes())
print(frame.toCsv())
"#,
        );
        assert_eq!(
            got,
            "      name   age  score\n0      Ana  30.0   1.50\n1  Lee, Jo  25.0    NaN\n2       Bo   NaN   3.25\nname         str\nage      float64\nscore    float64\ndtype: str\nname,age,score\nAna,30.0,1.5\n\"Lee, Jo\",25.0,\nBo,,3.25\n\n"
        );
    });
}

#[test]
fn a_missing_column_is_an_error_value_not_a_stop() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{Series, DataFrame, DataError}
function lookup() : void or Error with Effects {
    let frame := DataFrame([Series([1, 2], name = "v")])
    print(frame.col("v")?.sum())
    print(frame.col("nope")?.sum())
}
print(match lookup() { DataError.MissingColumn(name) => "no column " + name, _ => "ok" })
"#,
        );
        assert_eq!(got, "3\nno column nope\n");
    });
}

#[test]
fn text_and_html_show_long_frames_with_a_gap() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{Series, DataFrame}
let frame := DataFrame([Series(range(100).toList(), name = "n")])
print(frame)
print(frame.html().contains("<td>...</td>"))
"#,
        );
        assert!(got.contains("..."), "{got}");
        assert!(got.contains("[100 rows x 1 columns]"), "{got}");
        assert!(got.ends_with("true\n"), "{got}");
    });
}

#[test]
fn factorize_map_values_and_split_to_frame_keep_their_shapes() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{Series, DataFrame}
let s := Series(["b", "a", "b", "c"], name = "v")
let (codes, uniques) := s.factorize()
print(codes)
print(uniques)
print(s.mapValues(Map("a" => "A", "b" => "B")))
print(Series(["a-b", "c-d"]).splitToFrame("-"))
"#,
        );
        assert!(got.contains("0    0\n1    1\n2    0\n3    2"), "{got}");
        assert!(got.contains("0    b\n1    a\n2    c"), "{got}");
        assert!(
            got.contains("0      B\n1      A\n2      B\n3    NaN"),
            "{got}"
        );
        assert!(got.contains("0  a b\n1  c d"), "{got}");
    });
}

#[test]
fn csv_options_skip_rows_and_read_headerless_input() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{DataFrame}
let frame := DataFrame.parseCsv("x,y\n1,2\n3,4\n", header = false, skipRows = 2, nRows = 1)
print(frame)
function show() : void or Error with Effects {
    let quoted := DataFrame.parseCsv("name,note\nAna,\"hello\nworld\"\n")
    print(quoted.at(0, "note")?)
}
show()?
"#,
        );
        assert!(got.contains("   0  1\n0  3  4"), "{got}");
        assert!(got.contains("hello\nworld"), "{got}");
    });
}

#[test]
fn a_missing_group_is_an_error_value() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{Series, DataFrame, DataError}
function lookup() : void or Error with Effects {
    let frame := DataFrame([Series(["a", "b"], name = "k"), Series([1, 2], name = "v")])
    let grouped := frame.groupBy("k")?
    print(grouped.getGroup("z")?)
}
print(match lookup() { DataError.BadArgument(message) => message, _ => "ok" })
"#,
        );
        assert!(got.starts_with("no group with key z\n"), "{got}");
    });
}

#[test]
fn autocorrelation_matches_the_lagged_correlation_to_float_precision() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas.{Series}
let s := Series([3, 1, 2, 5, 4])
print(s.autocorr().asDouble())
"#,
        );
        let value: f64 = got.trim().parse().unwrap();
        assert!((value - 0.21380899352993954).abs() < 1e-12, "{got}");
    });
}

#[test]
fn read_csv_uses_the_native_path_and_falls_back_for_quotes() {
    on_big_stack(|| {
        let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let plain = dir.path().join("plain.csv");
        fs::write(
            &plain,
            "name,units,price,active\r\na,1,1.5,true\r\nb,2,2.5,false\r\nc,,3.0,true\r\n",
        )
        .unwrap();
        let quoted = dir.path().join("quoted.csv");
        fs::write(&quoted, "name,note\n\"a,b\",\"line one\nline two\"\n").unwrap();
        let source = format!(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {{
    let plain := kd.readCsv("{}")?
    print(plain.col("units")?.sum().asDouble())
    print(plain.col("price")?.sum().asDouble())
    print(plain.col("active")?.sum().asDouble())
    print(plain.col("name")?.toStrings().join("|"))
    let quoted := kd.readCsv("{}")?
    print(quoted.col("name")?.toStrings().join("|"))
    print(quoted.col("note")?.toStrings().join("|"))
}}
main()?
"#,
            plain.display(),
            quoted.display()
        );
        let got = run_program(&source);
        assert_eq!(got, "3.0\n7.0\n2.0\na|b|c\na,b\nline one\nline two\n");
    });
}

#[test]
fn nlargest_and_nsmallest_use_the_native_top_kernel() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let df := kd.DataFrame([kd.Series([5, 1, 3, 2, 4, 3], name = "v")])
    print(df.nlargest(3, "v")?.col("v")?.toInts())
    print(df.nsmallest(2, "v")?.col("v")?.toInts())
}
main()?
"#,
        );
        assert_eq!(got, "[5, 4, 3]\n[1, 2]\n");
    });
}

#[test]
fn a_left_merge_keeps_every_duplicate_right_row() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let left := kd.DataFrame([kd.Series([1, 2], name = "k"), kd.Series(["a", "b"], name = "l")])
    let rightKeys := [if i < 70 then 1 else 2 for i in range(73)]
    let rightValues := [if i < 70 then i else 100 + (i - 70) for i in range(73)]
    let right := kd.DataFrame([kd.Series(rightKeys, name = "k"), kd.Series(rightValues, name = "r")])
    let merged := left.merge(right, on = "k", how = "left")?
    print(merged.length())
    print(merged.col("r")?.toInts())
}
main()?
"#,
        );
        let expected: Vec<String> = (0..70).chain(100..103).map(|n| n.to_string()).collect();
        assert_eq!(got, format!("73\n[{}]\n", expected.join(", ")));
    });
}

#[test]
fn top_and_bottom_keep_the_earliest_row_of_a_tie() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let df := kd.DataFrame([
        kd.Series([0, 1, 2, 3, 4, 5, 6, 7, 8], name = "id"),
        kd.Series([4, 4, 5, 3, 5, 4, 5, 3, 4], name = "v"),
    ])
    print(df.nlargest(4, "v")?.col("id")?.toInts())
    print(df.nsmallest(4, "v")?.col("id")?.toInts())
}
main()?
"#,
        );
        assert_eq!(got, "[2, 4, 6, 0]\n[3, 7, 0, 1]\n");
    });
}

#[test]
fn pivot_count_accepts_non_numeric_columns() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let text := kd.DataFrame([
        kd.Series(["a", "a", "b", "b", "a"], name = "r"),
        kd.Series(["x", "y", "x", "x", "x"], name = "c"),
        kd.Series(["u", "v", "w", "q", "z"], name = "v"),
    ])
    print(text.pivotTable("r", "c", "v", "count")?)
    let flags := kd.DataFrame([
        kd.Series(["a", "a", "b"], name = "r"),
        kd.Series(["x", "y", "x"], name = "c"),
        kd.Series([true, false, true], name = "v"),
    ])
    print(flags.pivotTable("r", "c", "v", "count")?)
}
main()?
"#,
        );
        assert_eq!(
            got,
            "c  x    y\nr        \na  2  1.0\nb  2  NaN\nc  x    y\nr        \na  1  1.0\nb  1  NaN\n"
        );
    });
}

#[test]
fn grouped_integer_sums_stay_exact_above_two_to_the_fifty_third() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let df := kd.DataFrame([kd.Series(["a", "a"], name = "k"), kd.Series([9007199254740993, 2], name = "v")])
    print(df.groupBy("k")?.sum().col("v")?.toInts())
}
main()?
"#,
        );
        assert_eq!(got, "[9007199254740995]\n");
    });
}

#[test]
fn csv_column_inference_agrees_on_quoted_and_plain_text() {
    on_big_stack(|| {
        let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let plain = dir.path().join("plain.csv");
        fs::write(&plain, "x,y\n0x10,1\n2,3\n").unwrap();
        let quoted = dir.path().join("quoted.csv");
        fs::write(&quoted, "x,y\n\"0x10\",1\n2,3\n").unwrap();
        let source = format!(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {{
    let a := kd.readCsv("{}")?
    print(a.col("x")?.toStrings().join("|"))
    let b := kd.readCsv("{}")?
    print(b.col("x")?.toStrings().join("|"))
}}
main()?
"#,
            plain.display(),
            quoted.display()
        );
        assert_eq!(run_program(&source), "0x10|2\n0x10|2\n");
    });
}

#[test]
fn rolling_and_expanding_counts_validate_by_window_occupancy() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesdas as kd
function main() : void or Error with Effects {
    let s := kd.Series([1.5, 0.0 / 0.0, 3.25, 4.0], name = "m")
    print(s.rolling(3).count())
    print(s.expanding(3).count())
}
main()?
"#,
        );
        assert_eq!(
            got,
            "0    NaN\n1    NaN\n2    2.0\n3    2.0\nName: m, dtype: float64\n0    NaN\n1    NaN\n2    2.0\n3    3.0\nName: m, dtype: float64\n"
        );
    });
}
