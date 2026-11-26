fn main() {
    let mut s = String::new();
    let mut i = 0;
    while i < 100_000 {
        s.push_str("abc");
        if i % 10 == 0 {
            s.push('|');
        }
        i += 1;
    }
    let extra = format!("[{}]", s.len());
    s.push_str(&extra);
    println!("len = {}, extra = {}, trimmed = {}", s.len(), extra, "  pad  ".trim());
}
