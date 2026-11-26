fn main() {
    let mut s = 0;
    for i in 1..=10 {
        if i % 2 == 0 {
            continue;
        }
        s += i;
    }
    println!("s = {}", s);
    let mut n = 0;
    while n < 5 {
        n += 1;
    }
    println!("n = {}", n);
    let mut k = 0;
    let val = loop {
        k += 1;
        if k == 7 {
            break k * 2;
        }
    };
    println!("k = {}, val = {}", k, val);
    let mut grid = 0;
    for i in 0..3 {
        for j in 0..3 {
            if i == j {
                grid += 1;
            }
        }
    }
    println!("grid = {}", grid);
    let mut x = 0;
    loop {
        x += 1;
        if x >= 4 {
            break;
        }
    }
    println!("x = {}", x);
}
