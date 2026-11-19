fn main() {
    let mut values: Vec<i64> = Vec::new();
    let mut x: i64 = 42;
    for _ in 0..2_000_000 {
        x = (x * 1103515245 + 12345) % 2147483648;
        values.push(x);
    }
    values.sort_unstable();
    let total: i64 = values.iter().step_by(1000).sum();
    println!("{}", total);
}
