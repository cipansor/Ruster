fn down(n: i64) -> i64 {
    if n == 0 {
        0
    } else {
        1 + down(n - 1)
    }
}

fn main() {
    println!("{}", down(100_000));
}
