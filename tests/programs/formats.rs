fn main() {
    println!("|{:>8}|{:<8}|{:^8}|", "right", "left", "mid");
    println!("|{:08}|{:+}|{:+}|", 42, 7, -7);
    println!("{:x} {:X} {:b} {:o}", 255, 255, 10, 8);
    println!("{:.2} {:.0} {}", 3.14159, 2.5, 1.0);
    println!("{:?} {:?} {:?}", "s", 'c', vec![1, 2]);
    println!("{{literal}} {}", 1);
    println!("{:>7.2}", 3.14159);
    println!("{0} {1} {0}", "a", "b");
    println!("{:5}", true);
}
