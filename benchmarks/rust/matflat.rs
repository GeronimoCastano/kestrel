fn run(w: usize, iterations: usize) -> f64 {
    let size = w * w;
    let mut a = vec![0.0f64; size];
    let mut b = vec![0.0f64; size];
    for i in 0..w {
        for j in 0..w {
            a[i * w + j] = ((i * 17 + j * 31) % 100) as f64;
        }
    }
    for _ in 0..iterations {
        for i in 1..w - 1 {
            for j in 1..w - 1 {
                b[i * w + j] = 0.25
                    * (a[(i - 1) * w + j]
                        + a[(i + 1) * w + j]
                        + a[i * w + j - 1]
                        + a[i * w + j + 1]);
            }
        }
        b[..w].copy_from_slice(&a[..w]);
        let last = (w - 1) * w;
        b[last..last + w].copy_from_slice(&a[last..last + w]);
        for i in 1..w - 1 {
            b[i * w] = a[i * w];
            b[i * w + w - 1] = a[i * w + w - 1];
        }
        std::mem::swap(&mut a, &mut b);
    }
    let mut total = 0.0;
    for value in a {
        total += value;
    }
    total
}

fn main() {
    println!("{:.1}", run(700, 20));
}
