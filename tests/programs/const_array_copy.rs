const A: [i64; 1] = [0];

fn next_val() -> i64 {
    let mut a = A;
    a[0] += 1;
    a[0]
}

fn main() {
    println!("{} {}", next_val(), next_val());
}
