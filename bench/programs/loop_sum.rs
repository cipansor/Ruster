fn main() {
    let n: i64 = 10_000_000;
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        acc += (i % 7) * 3 + 1;
        i += 1;
    }
    println!("acc = {}", acc);
}
