fn main() {
    let n: i64 = 1_000_000;
    let mut is_prime = vec![true; (n + 1) as usize];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i: i64 = 2;
    while i * i <= n {
        if is_prime[i as usize] {
            let mut j = i * i;
            while j <= n {
                is_prime[j as usize] = false;
                j += i;
            }
        }
        i += 1;
    }
    let mut count = 0;
    for k in 0..=n {
        if is_prime[k as usize] {
            count += 1;
        }
    }
    println!("primes <= {}: {}", n, count);
}
