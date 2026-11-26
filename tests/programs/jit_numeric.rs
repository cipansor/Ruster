fn clamp01(x: i64) -> i64 {
    if x < 0 {
        0
    } else if x > 100 {
        100
    } else {
        x
    }
}

fn mix(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64) -> i64 {
    let t = (a + b) * (c - d) + (e << 2) - (f >> 1);
    if (t & 7) == 3 {
        t ^ 0x55
    } else {
        t | 1
    }
}

fn sum_squares(n: i64) -> i64 {
    let mut s = 0;
    let mut i = 0;
    while i <= n {
        s += i * i;
        i += 1;
    }
    s
}

fn collatz_steps(mut n: i64) -> i64 {
    let mut steps = 0;
    while n != 1 {
        if (n & 1) == 0 {
            n = n >> 1;
        } else {
            n = 3 * n + 1;
        }
        steps += 1;
    }
    steps
}

fn six_locals() -> i64 {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    a + b + c + d + e + f
}

fn main() {
    println!("{} {}", mix(1, 2, 3, 4, 5, 6), clamp01(-5));
    println!("{} {}", sum_squares(1000), collatz_steps(27));
    println!("{}", six_locals());
}
