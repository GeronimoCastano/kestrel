use kestrel::parse_file;
use std::{hint::black_box, time::Instant};

fn main() {
    let source = include_str!("../testdata/language/rules.kes");
    let iterations: usize = std::env::var("KESTREL_BENCH_ITERATIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(10_000);
    assert!(parse_file("rules.kes", source).is_ok());
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(parse_file("rules.kes", black_box(source)));
    }
    let elapsed = start.elapsed();
    println!(
        "workload=examples/rules.kes bytes={} iterations={iterations} elapsed_seconds={:.6} MiB_per_second={:.2}",
        source.len(),
        elapsed.as_secs_f64(),
        source.len() as f64 * iterations as f64 / elapsed.as_secs_f64() / 1_048_576.0
    );
    println!(
        "Includes source ownership, lexing, parsing and destruction. Not generated-code performance."
    );
}
