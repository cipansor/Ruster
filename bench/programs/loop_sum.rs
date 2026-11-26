fn work(n: i64) -> i64 {
    let mut acc = 0;
    let mut i = 0;
    while i < n {
        acc += (i & 7) * 3 + 1;
        i += 1;
    }
    acc
}

fn main() {
    println!("acc = {}", work(10_000_000));
}
