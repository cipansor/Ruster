fn main() {
    let mut v = vec![5, 3, 9, 1];
    v.push(7);
    v.sort();
    let mut total = 0;
    for x in &v {
        total += x;
    }
    println!("{:?} {} {} {}", v, total, v.len(), v.contains(&9));
    let mut grid = vec![vec![0i64; 3]; 2];
    grid[1][2] = 42;
    println!("{:?}", grid);
    let mut names = Vec::new();
    names.push("a".to_string());
    names.push(String::from("b"));
    println!("{}", names.join("+"));
    v.reverse();
    println!("{} {}", v[0], v[v.len() - 1]);
    let mut w = Vec::new();
    for i in 0..5 {
        if i % 2 == 0 {
            w.push(i);
        }
    }
    println!("{:?} len = {}", w, w.len());
}
