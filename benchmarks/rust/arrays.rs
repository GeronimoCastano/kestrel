fn main() {
    let n: usize = 5_000_000;
    let mut values: Vec<i64> = Vec::new();
    for i in 0..n {
        values.push((i % 1000) as i64);
    }
    let mut total: i64 = 0;
    for i in 0..values.len() {
        total += values[i];
    }
    for _ in 0..n / 2 {
        total += values.pop().unwrap();
    }
    println!("{}", total);
}
