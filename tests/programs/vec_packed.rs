fn main() {
    let mut v = vec![0i64; 5];
    v[2] = 7;
    v[4] = v[2] + 1;
    let mut sum = 0i64;
    let mut i = 0usize;
    while i < 5 {
        sum += v[i];
        i += 1;
    }
    println!("{} {}", sum, v.len());
    v.reverse();
    println!("{} {}", v[0], v[4]);
    v.push(42);
    v[1] = 8;
    println!("{} {} {}", v[0], v[1], v.len());

    let mut b = vec![true; 4];
    b[1] = false;
    let mut t = 0i64;
    let mut j = 0usize;
    while j < 4 {
        if b[j] {
            t += 1;
        }
        j += 1;
    }
    println!("{} {}", t, b.len());

    let m = vec![vec![0i64; 2]; 2];
    println!("{} {}", m.len(), m[1].len());

    let names = vec!["a", "b"];
    println!("{} {}", names[0], names.len());
}
