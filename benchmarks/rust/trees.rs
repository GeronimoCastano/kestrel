// The Benchmarks Game binary-trees program, single-threaded.
enum Tree {
    Leaf,
    Node(Box<Tree>, Box<Tree>),
}

fn make(depth: i64) -> Tree {
    if depth == 0 {
        Tree::Node(Box::new(Tree::Leaf), Box::new(Tree::Leaf))
    } else {
        Tree::Node(Box::new(make(depth - 1)), Box::new(make(depth - 1)))
    }
}

fn check(t: &Tree) -> i64 {
    match t {
        Tree::Leaf => 0,
        Tree::Node(l, r) => 1 + check(l) + check(r),
    }
}

fn main() {
    let max_depth = 18;
    let stretch = max_depth + 1;
    println!("stretch tree of depth {stretch}\t check: {}", check(&make(stretch)));
    let long_lived = make(max_depth);
    let mut depth = 4;
    while depth <= max_depth {
        let iterations = 1i64 << (max_depth - depth + 4);
        let mut total = 0;
        for _ in 0..iterations {
            total += check(&make(depth));
        }
        println!("{iterations}\t trees of depth {depth}\t check: {total}");
        depth += 2;
    }
    println!("long lived tree of depth {max_depth}\t check: {}", check(&long_lived));
}
