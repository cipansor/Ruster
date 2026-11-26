fn steps(mut n: i64) -> i64 {
    let mut k = 0;
    while n != 1 {
        if (n & 1) == 0 {
            n = n >> 1;
        } else {
            n = 3 * n + 1;
        }
        k += 1;
    }
    k
}

fn main() {
    let mut total = 0;
    let mut i = 1;
    while i <= 200_000 {
        total += steps(i);
        i += 1;
    }
    println!("total = {}", total);
}
