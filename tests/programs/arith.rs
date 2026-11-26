fn main() {
    let a: i64 = 7;
    let b: i64 = 3;
    println!("{} {} {} {} {}", a + b, a - b, a * b, a / b, a % b);
    println!("{} {} {}", a & b, a | b, a ^ b);
    println!("{} {} {}", a << 2, a >> 1, -a);
    println!("{} {}", a == 7, b > 5);
    let f = 3.5;
    println!("{:.3} {:.1} {}", f * 2.0, f / 2.0, 3.0 + 1.0);
    let big = 300i64 as u8;
    println!("{} {}", big, 200u8 as i64 + 100);
    println!("{} {}", 'A' as i64, 66 as u8 as char);
    println!("{}", 1 + 2 * 3 - 4 / 2);
    println!("{} {}", true && false, true || false);
    println!("{}", 0xff + 0b1010 + 0o17);
}
