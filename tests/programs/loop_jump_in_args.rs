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

    // A `break` while an index expression is half-built. The first pass indexes
    // normally so the second pass actually reaches `break arr[0]` with the array
    // operand still on the stack.
    let arr = [10, 20, 30];
    let mut k = 0;
    let picked = loop {
        k += 1;
        arr[if k == 2 { break arr[0]; } else { 1 }];
        if k > 10 {
            break 999;
        }
    };
    println!("{} {}", k, picked);

    // A semicolon-terminated `break value;` still diverges, so the `if` merge
    // must take its depth from the `else` branch.
    let semi_break = 10 + loop {
        add(1, if false { break 2; } else { 3 });
        add(4, { break 5; });
    };
    println!("{}", semi_break);

    // A skipped inner loop whose body contains `return` must restore the
    // enclosing depth before the outer `break`.
    let skipped = 10 + loop {
        while false {
            return;
        }
        add(1, { break 2; });
    };
    println!("{}", skipped);

    // An `else if` arm that calls `process::exit` cannot fall through, so the
    // merge must take its depth from the final `else`.
    let exit_arm = 10 + loop {
        add(
            if false { break 2; } else if false { std::process::exit(1) } else { 3 },
            { break 5; },
        );
    };
    println!("{}", exit_arm);

    // Untaken `return`s in the other loop forms must restore the depth before
    // an outer `break` discards a live argument.
    let loop_exit = 10 + loop {
        add(1, { loop { break 2; return; } });
        add(4, { break 7; });
    };
    let range_exit = 10 + loop {
        for i in 0..0 {
            return;
        }
        add(4, { break 7; });
    };
    let coll_exit = 10 + loop {
        let empty: Vec<i64> = Vec::new();
        for x in empty {
            return;
        }
        add(4, { break 7; });
    };
    println!("{} {} {}", loop_exit, range_exit, coll_exit);

    // A `break` in a method argument discards the receiver operand too.
    let mut v: Vec<i64> = Vec::new();
    let got = loop {
        v.push(add(1, if true { break 42; } else { 0 }));
    };
    println!("{} {}", got, v.len());
}
