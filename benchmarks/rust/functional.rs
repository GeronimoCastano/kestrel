fn main() {
    let n: i64 = 5_000_000;
    let total: i64 = (0..n)
        .filter(|x| x % 2 == 0)
        .map(|x| x * 2)
        .map(|x| x + 7)
        .filter(|x| x % 3 == 0)
        .sum();
    println!("{}", total);
}
