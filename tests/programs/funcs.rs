fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn sum_to(n: i64) -> i64 {
    let mut s = 0;
    let mut i = 1;
    while i <= n {
        s += i;
        i += 1;
    }
    s
}

fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{} {} {}", gcd(48, 18), sum_to(100), add(2, 3));
    println!("fib = {}", fib(20));
}
