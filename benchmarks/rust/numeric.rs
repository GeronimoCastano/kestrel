fn lcg(n: i64) -> i64 {
    let mut x: i64 = 42;
    let mut total: i64 = 0;
    for _ in 0..n {
        x = (x * 1103515245 + 12345) % 2147483648;
        total += x % 1000;
    }
    total
}

fn leibniz(n: i64) -> f64 {
    let mut sum = 0.0;
    let mut sign = 1.0;
    for i in 0..n {
        sum += sign / (2.0 * i as f64 + 1.0);
        sign = -sign;
    }
    4.0 * sum
}

fn main() {
    println!("{}", lcg(50_000_000));
    println!("{:.9}", leibniz(50_000_000));
}
