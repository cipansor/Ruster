fn add(x: i64, y: i64) -> i64 {
    x + y
}

fn pick() -> i64 {
    add(1, if true { return 2; } else { 3 })
}

fn main() {
    println!("{} {}", 9, pick());
}
