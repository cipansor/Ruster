fn divmod(a: i64, b: i64) -> (i64, i64) {
    (a / b, a % b)
}

fn main() {
    let (q, r) = divmod(17, 5);
    let t = (1, 2.5, true);
    println!("{} {} {} {} {}", q, r, t.0, t.1, t.2);
}
