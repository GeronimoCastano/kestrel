//! std.kesplot: TeX labels, SVG/PNG/PDF/JPEG output, styles and Kesdas adapters.
//! These tests compile and run real Kestrel programs, then inspect the files.

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

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn tex_available() -> bool {
    command_exists("latex") && command_exists("dvisvgm")
}

fn graphics_available() -> bool {
    command_exists("rsvg-convert")
}

#[test]
fn science_line_plot_uses_tex_and_writes_well_formed_svg() {
    if !tex_available() {
        return;
    }
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt
import std.math.{sin}

function render() : String or Error with Effects {
    let fig := plt.Figure(style = plt.Style.science())
    let xs := [i.toDouble() / 10.0 for i in range(21)]
    let ys := [sin(x) for x in xs]
    fig.plot(xs, ys, label = "signal", marker = "o")
    fig.title("Demo $x^2$")
    fig.xLabel("time (s)")
    fig.yLabel("$\\sigma$")
    fig.legend()
    fig.toSvg()
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
})
"#,
        );
        assert!(got.starts_with("<?xml version=\"1.0\""), "{got}");
        assert!(
            got.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""),
            "{got}"
        );
        assert!(got.trim_end().ends_with("</svg>"), "{got}");
        assert!(got.contains("stroke=\"#0C5DA5\""), "science cycle used");
        assert!(
            got.contains(">signal</text>"),
            "legend text remains normal text"
        );
        assert!(!got.contains("data-kestex"), "TeX labels are resolved");
        assert!(
            got.contains("<g transform="),
            "TeX labels are vector groups"
        );
        assert!(
            !got.contains(">Demo x²</text>"),
            "Unicode math fallback is gone"
        );
        assert!(got.matches("<path").count() > 10, "paths exist");
        assert!(!got.contains("NaN"), "SVG coordinates are finite");
        assert!(!got.contains("inf"), "SVG coordinates are finite");
    });
}

#[test]
fn every_named_style_renders_a_complete_figure() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot.{Figure, Style}

function report() : String or Error with Effects {
    let mut text := ""
    for (name in Style.names()) {
        let style := match Style.byName(name) {
            found => found,
            _ => Style.standard(),
        }
        let fig := Figure(4.0, 3.0, style)
        fig.plot([0.0, 1.0, 2.0], [0.0, 1.0, 0.0])
        fig.title(name)
        text = text + name + "=" + fig.toSvg()?.length().text() + "\n"
    }
    text
}

print(match report() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
})
"#,
        );
        let lines: Vec<&str> = got.trim_end().lines().collect();
        assert_eq!(lines.len(), 27, "{got}");
        for line in lines {
            let (_, size) = line.rsplit_once('=').unwrap();
            let size: usize = size.parse().unwrap();
            assert!(size > 1000, "style output is complete: {line}");
        }
    });
}

#[test]
fn kesdas_frames_render_line_bar_scatter_hist_box_heatmap_and_pie() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt
import std.kesdas.{Series, DataFrame}

function render() : String or Error with Effects {
    let frame := DataFrame([
        Series(["north", "south", "north", "east"], name = "region"),
        Series([120, 80, 95, 60], name = "units"),
        Series([2.5, 3.0, 2.5, 4.0], name = "price"),
    ])

    let line := plt.Figure(style = plt.Style.science())
    line.plotFrame(frame, x = "region", y = ["units", "price"], marker = "o")?
    line.title("frame line")

    let bars := plt.Figure(style = plt.Style.science())
    bars.barFrame(frame, x = "region", y = ["units", "price"], horizontal = true)?

    let scatter := plt.Figure(style = plt.Style.science())
    scatter.scatterFrame(frame, x = "units", y = "price", colorBy = "region")?

    let hist := plt.Figure(style = plt.Style.science())
    hist.histFrame(frame)?

    let box := plt.Figure(style = plt.Style.science())
    box.boxFrame(frame)?

    let heat := plt.Figure(style = plt.Style.science())
    heat.heatmapFrame(frame)?

    let pie := plt.Figure(style = plt.Style.science())
    pie.pieFrame(frame, "units", "region", autoPercent = true)?

    line.toSvg()? + bars.toSvg()? + scatter.toSvg()? + hist.toSvg()? + box.toSvg()? + heat.toSvg()? + pie.toSvg()?
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR: unknown",
})
"#,
        );
        assert!(!got.contains("ERROR"), "{got}");
        assert!(got.matches("<svg").count() == 7, "seven figures");
        assert!(got.matches("</svg>").count() == 7, "seven figures close");
        assert!(got.contains(">frame line</text>"), "{got}");
        assert!(got.contains(">north</text>"), "categories appear");
        assert!(got.contains(">units</text>"), "column labels appear");
        assert!(
            got.contains(">region</text>"),
            "x column becomes the axis label"
        );
        assert!(got.contains("%</text>"), "pie percentages appear");
        assert!(got.contains("clipPath"), "{got}");
    });
}

#[test]
fn contours_and_tex_labels_render() {
    if !tex_available() {
        return;
    }
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt
import std.math.{sin, cos}

function render() : String or Error with Effects {
    let rows := 18
    let cols := 20
    let matrix := [[sin(r.toDouble() / 3.0) * cos(c.toDouble() / 4.0) for c in range(cols)] for r in range(rows)]
    let fig := plt.Figure(style = plt.Style.science())
    fig.contour(matrix, 5, "viridis", 1.0, [], [], true, "$z$")
    fig.title("$\\alpha$ field $x^2$")
    fig.toSvg()
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
})
"#,
        );
        assert!(!got.contains("ERROR"), "{got}");
        assert!(!got.contains("data-kestex"), "TeX labels are resolved");
        assert!(
            got.contains("<g transform="),
            "math is embedded as vector paths"
        );
        assert!(
            got.matches("<path").count() > 20,
            "contour and glyph paths exist"
        );
        assert!(!got.contains("NaN"), "{got}");
    });
}

#[test]
fn broken_tex_is_reported_instead_of_silently_falling_back() {
    if !tex_available() {
        return;
    }
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt

function render() : String or Error with Effects {
    let fig := plt.Figure(style = plt.Style.science())
    fig.plot([0.0, 1.0], [1.0, 0.0])
    fig.title("$\\notacommand$")
    fig.toSvg()
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => message,
    _ => "no error",
})
"#,
        );
        assert!(got.contains("Cannot typeset math"), "{got}");
        assert!(got.contains("ndefined"), "TeX error detail is kept: {got}");
    });
}

#[test]
fn html_embeds_tex_and_png_pdf_jpeg_are_written() {
    if !tex_available() || !graphics_available() {
        return;
    }
    on_big_stack(|| {
        let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let root = dir.path().display();
        let source = format!(
            r#"
import std.kesplot as plt

function go() : void or Error with Effects {{
    let fig := plt.Figure(style = plt.Style.science())
    fig.style.dpi = 100
    fig.plot([0.0, 1.0, 2.0], [0.0, 1.0, 0.0])
    fig.title("$x^2$ and $\\frac{{a}}{{b}}$")
    fig.xLabel("$\\theta$")
    fig.save("{root}/figure.svg")?
    fig.save("{root}/figure.html")?
    fig.save("{root}/figure.png")?
    fig.save("{root}/figure.pdf")?
    fig.save("{root}/figure.jpg")?
    fig.save("{root}/figure.jpeg")?
}}

print(match go() {{
    _ => "ok",
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
}})
"#,
        );
        let got = run_program(&source);
        assert_eq!(got.trim(), "ok");
        for name in [
            "figure.svg",
            "figure.html",
            "figure.png",
            "figure.pdf",
            "figure.jpg",
            "figure.jpeg",
        ] {
            let path = dir.path().join(name);
            let bytes = fs::read(&path).unwrap();
            assert!(bytes.len() > 1000, "{name} is nonempty");
        }
        let svg = fs::read_to_string(dir.path().join("figure.svg")).unwrap();
        assert!(!svg.contains("data-kestex"), "saved SVG uses TeX");
        assert!(svg.contains("<g transform="), "saved SVG has TeX paths");
        let html = fs::read_to_string(dir.path().join("figure.html")).unwrap();
        assert!(html.starts_with("<!doctype html>"), "{html}");
        assert!(html.contains("<g transform="), "HTML uses TeX");
        let png = fs::read(dir.path().join("figure.png")).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert!(width >= 300 && height >= 200, "PNG size {width}x{height}");
        let pdf = fs::read(dir.path().join("figure.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "PDF signature");
        let jpg = fs::read(dir.path().join("figure.jpg")).unwrap();
        assert_eq!(&jpg[..3], b"\xff\xd8\xff", "JPEG signature");
        let jpeg = fs::read(dir.path().join("figure.jpeg")).unwrap();
        assert_eq!(&jpeg[..3], b"\xff\xd8\xff", "JPEG signature");
    });
}

#[test]
fn paths_with_spaces_and_quotes_write_all_formats() {
    if !tex_available() || !graphics_available() {
        return;
    }
    on_big_stack(|| {
        let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let nested = dir.path().join("with space's");
        fs::create_dir(&nested).unwrap();
        let source = format!(
            r#"
import std.kesplot as plt

function go() : void or Error with Effects {{
    let fig := plt.Figure(style = plt.Style.science())
    fig.plot([0.0, 1.0], [0.0, 1.0])
    fig.title("$\\sqrt{{x}}$")
    fig.save("{}")?
    fig.save("{}")?
    fig.save("{}")?
}}

print(match go() {{
    _ => "ok",
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
}})
"#,
            nested.join("a figure.svg").display(),
            nested.join("a figure.png").display(),
            nested.join("a figure.pdf").display(),
        );
        let got = run_program(&source);
        assert_eq!(got.trim(), "ok");
        for name in ["a figure.svg", "a figure.png", "a figure.pdf"] {
            assert!(nested.join(name).exists(), "{name} exists");
        }
        assert!(
            fs::read(nested.join("a figure.png"))
                .unwrap()
                .starts_with(b"\x89PNG")
        );
        assert!(
            fs::read(nested.join("a figure.pdf"))
                .unwrap()
                .starts_with(b"%PDF-")
        );
    });
}

#[test]
fn dense_categories_rotate_and_empty_math_is_not_a_placeholder() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt

function render() : String or Error with Effects {
    let names := ["category " + i.text() for i in range(24)]
    let values := [i.toDouble() for i in range(24)]
    let fig := plt.Figure(style = plt.Style.standard())
    fig.bar(names, values)
    fig.title("A deliberately long title that has to wrap rather than leave the canvas")
    fig.xLabel("$$")
    fig.toSvg()
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
})
"#,
        );
        assert!(!got.contains("ERROR"), "{got}");
        assert!(
            !got.contains("data-kestex"),
            "empty math leaves no placeholder"
        );
        assert!(got.contains("rotate(-90"), "dense labels rotate vertically");
        assert!(got.contains("A deliberately long title"), "{got}");
    });
}

#[test]
fn zero_sized_pie_reports_no_data() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt

function render() : String or Error with Effects {
    let fig := plt.Figure(style = plt.Style.standard())
    fig.pie([0.0, 0.0], ["a", "b"])
    fig.toSvg()
}

print(match render() {
    text => text,
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
})
"#,
        );
        assert!(got.contains(">no data</text>"), "{got}");
    });
}

#[test]
fn save_writes_an_svg_file() {
    on_big_stack(|| {
        let dir = native::TemporaryDirectory::new(&std::env::temp_dir()).unwrap();
        let target = dir.path().join("figure.svg");
        let source = format!(
            r#"
import std.kesplot as plt

function go() : void or Error with Effects {{
    let fig := plt.Figure(style = plt.Style.science())
    fig.plot([0.0, 1.0, 2.0], [0.0, 1.0, 0.0])
    fig.title("written")
    fig.save("{}")?
}}

print(match go() {{
    _ => "wrote",
    RuntimeError.Message(message) => "ERROR: " + message,
    _ => "ERROR",
}})
"#,
            target.display()
        );
        let got = run_program(&source);
        assert_eq!(got.trim(), "wrote");
        let text = fs::read_to_string(&target).unwrap();
        assert!(text.starts_with("<?xml"), "{text}");
        assert!(text.contains("<svg"), "{text}");
        assert!(text.contains(">written</text>"), "{text}");
        assert!(text.trim_end().ends_with("</svg>"), "{text}");
        assert!(text.matches('\n').count() > 5);
    });
}

#[test]
fn unsupported_extension_is_rejected() {
    on_big_stack(|| {
        let got = run_program(
            r#"
import std.kesplot as plt

function go() : String or Error with Effects {
    let fig := plt.Figure()
    fig.save("/tmp/kesplot-test-output.gif")?
    "saved"
}

print(match go() {
    text => text,
    RuntimeError.Message(message) => message,
    _ => "ERROR",
})
"#,
        );
        assert!(got.contains("saves .svg"), "{got}");
    });
}

#[test]
fn the_example_program_runs() {
    on_big_stack(|| {
        let source = include_str!("../testdata/language/kesplot.kes");
        let got = run_program(source);
        assert!(got.contains("kesplot wrote"), "{got}");
        assert!(got.contains("Figure("), "{got}");
        let path = Path::new("kesplot-example.svg");
        if path.exists() {
            fs::remove_file(path).ok();
        }
    });
}
