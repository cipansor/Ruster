fn down(n: i64) -> i64 {
    if n == 0 {
        0
    } else {
        1 + down(n - 1)
    }
}

// Mutual recursion: neither function may be JIT-compiled, because a cycle runs
// through a second function rather than calling itself directly.
fn even(n: i64) -> i64 {
    if n == 0 {
        1
    } else {
        odd(n - 1)
    }
}

fn odd(n: i64) -> i64 {
    if n == 0 {
        0
    } else {
        even(n - 1)
    }
}

fn main() {
    println!("{}", down(100_000));
    println!("{} {}", even(10_000), odd(10_001));
}
