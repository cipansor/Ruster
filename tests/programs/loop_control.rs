fn main() {
    let mut i = 0i64;
    let mut sum = 0i64;
    while i < 20 {
        i += 1;
        if i % 2 == 0 {
            continue;
        }
        if i > 15 {
            break;
        }
        sum += i;
    }
    println!("{} {}", i, sum);

    let mut outer = 0i64;
    let mut count = 0i64;
    while outer < 3 {
        let mut inner = 0i64;
        while inner < 5 {
            inner += 1;
            if inner == 2 {
                continue;
            }
            if inner == 4 {
                break;
            }
            count += 1;
        }
        outer += 1;
    }
    println!("{}", count);
}
