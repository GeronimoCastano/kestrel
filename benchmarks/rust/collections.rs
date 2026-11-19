use std::collections::HashMap;

fn build(n: i64) -> i64 {
    let mut m: HashMap<i64, i64> = HashMap::new();
    for i in 0..n {
        m.insert(i, i * 2);
    }
    let total: i64 = m.values().sum();
    let items: Vec<i64> = m.iter().map(|(k, v)| v + k).collect();
    let list_total: i64 = items.iter().sum();
    total + list_total
}

fn main() {
    println!("{}", build(2_000_000));
}
