fn classify(n: i64) -> &'static str {
    if n < 0 {
        "neg"
    } else if n == 0 {
        "zero"
    } else {
        "pos"
    }
}

fn main() {
    let x = 5;
    let x = x * 2;
    {
        let x = x + 1;
        println!("inner {}", x);
    }
    println!("outer {}", x);
    println!("{} {} {}", classify(-3), classify(0), classify(8));
    let mut acc = 0;
    for i in 0..100 {
        if i > 50 {
            break;
        }
        if i % 3 == 0 {
            continue;
        }
        acc += i;
    }
    println!("acc = {}", acc);
    let mut nested = 0;
    for i in 0..4 {
        let mut j = 0;
        while j < i {
            nested += i * j;
            j += 1;
        }
    }
    println!("nested = {}", nested);
}
