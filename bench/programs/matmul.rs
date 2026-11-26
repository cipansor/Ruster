fn main() {
    let n: usize = 100;
    let mut a = vec![vec![0i64; n]; n];
    let mut b = vec![vec![0i64; n]; n];
    let mut seed: i64 = 12345;
    for i in 0..n {
        for j in 0..n {
            seed = (seed * 1103515245 + 12345) % 2147483648;
            a[i][j] = seed % 100;
            seed = (seed * 1103515245 + 12345) % 2147483648;
            b[i][j] = seed % 100;
        }
    }
    let mut c = vec![vec![0i64; n]; n];
    for i in 0..n {
        for j in 0..n {
            let mut sum: i64 = 0;
            for k in 0..n {
                sum += a[i][k] * b[k][j];
            }
            c[i][j] = sum;
        }
    }
    println!("c[0][0] = {}, c[n-1][n-1] = {}", c[0][0], c[n - 1][n - 1]);
}
