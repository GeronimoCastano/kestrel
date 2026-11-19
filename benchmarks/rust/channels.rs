use std::sync::mpsc;
use std::thread;

fn run(n: i64) -> i64 {
    let (sender, receiver) = mpsc::channel::<i64>();
    thread::scope(|scope| {
        scope.spawn(move || {
            for i in 0..n {
                sender.send(i).unwrap();
            }
        });
        let consumer = scope.spawn(move || receiver.iter().sum::<i64>());
        consumer.join().unwrap()
    })
}

fn main() {
    println!("{}", run(5_000_000));
}
