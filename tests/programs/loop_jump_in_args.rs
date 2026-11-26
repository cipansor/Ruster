fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    // A `break` inside an argument discards the argument already pushed for
    // this call; it must not be left for the `+` after the loop.
    let then_break = 10 + loop {
        add(1, if true { break 2; } else { 3 });
    };
    let else_break = 10 + loop {
        add(1, if false { 3 } else { break 2; });
    };
    println!("{} {}", then_break, else_break);

    // A `continue` inside an argument must not accumulate operands per pass.
    let mut i = 0;
    let mut sum = 0;
    loop {
        i += 1;
        if i > 5 {
            break;
        }
        sum += add(1, if i == 3 { continue; } else { i });
    }
    println!("{} {}", i, sum);

    // Same, with a `for` iterator on the operand stack.
    let mut total = 0;
    for n in 0..5 {
        total += add(1, if n == 2 { continue; } else { n });
    }
    println!("{}", total);

    // A `break` while an index expression is half-built.
    let arr = [10, 20, 30];
    let mut k = 0;
    let picked = loop {
        k += 1;
        arr[if k == 2 { break arr[0]; } else { 1 }];
        break 999;
    };
    println!("{} {}", k, picked);

    // A `break` in a method argument discards the receiver operand too.
    let mut v: Vec<i64> = Vec::new();
    let got = loop {
        v.push(add(1, if true { break 42; } else { 0 }));
    };
    println!("{} {}", got, v.len());
}
