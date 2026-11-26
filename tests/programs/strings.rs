fn main() {
    let mut s = String::from("hello");
    s.push_str(", world");
    s.push('!');
    println!("{} {} {}", s, s.len(), s.is_empty());
    println!("{}", s.to_uppercase());
    println!("{}", s.replace("world", "rust"));
    println!("{}", "  trim me  ".trim());
    println!("{} {}", "abc".starts_with("ab"), "abc".ends_with("bc"));
    println!("{} {}", "abcdef".contains("cd"), "abc".repeat(3));
    println!("{}", format!("{}-{}", 1, 2));
    let chars = "xyz".chars();
    for c in chars {
        print!("{} ", c);
    }
    println!("");
    println!("{:?}", "quoted");
}
