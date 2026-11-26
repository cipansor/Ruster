fn main() {
    let mut a = vec![0i64; 4];
    let b = vec![10i64, 20, 30, 40];
    let mut i = 0usize;
    while i < 4 {
        a[i] = b[i];
        i += 1;
    }
    let mut j = 0usize;
    while j < 4 {
        let k = 3 - j;
        a[j] = a[k];
        j += 1;
    }
    println!("{} {} {} {}", a[0], a[1], a[2], a[3]);
}
