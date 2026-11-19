fn build(n: i64) -> usize {
    let mut s = String::new();
    for i in 0..n {
        s.push_str(&i.to_string());
        s.push(',');
    }
    s.chars().count()
}

fn main() {
    println!("{}", build(2_000_000));
}
