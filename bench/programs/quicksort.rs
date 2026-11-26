fn quicksort(v: &mut Vec<i64>, lo: i64, hi: i64) {
    if lo >= hi {
        return;
    }
    let pivot = v[lo as usize];
    let mut i = lo;
    let mut j = hi;
    while i < j {
        while i < j && v[j as usize] >= pivot {
            j -= 1;
        }
        v[i as usize] = v[j as usize];
        while i < j && v[i as usize] <= pivot {
            i += 1;
        }
        v[j as usize] = v[i as usize];
    }
    v[i as usize] = pivot;
    quicksort(v, lo, i - 1);
    quicksort(v, i + 1, hi);
}

fn main() {
    let n: usize = 50_000;
    let mut v = vec![0i64; n];
    let mut seed: i64 = 42;
    for i in 0..n {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        v[i] = seed % 100000;
    }
    quicksort(&mut v, 0, (n - 1) as i64);
    let mut ok = true;
    for i in 1..n {
        if v[i - 1] > v[i] {
            ok = false;
        }
    }
    println!("sorted = {}, first = {}, last = {}", ok, v[0], v[n - 1]);
}
