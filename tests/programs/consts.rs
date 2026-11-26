const LIMIT: i64 = 1000;
const STEP: i64 = LIMIT / 10;
static mut COUNTER: i64 = 5;

fn main() {
    let mut sum = 0;
    let mut i = 0;
    while i < LIMIT {
        sum += STEP;
        i += STEP;
    }
    unsafe {
        COUNTER = COUNTER + 1;
    }
    let counter = unsafe { COUNTER };
    println!("{} {} {}", sum, counter, i64::MAX);
    println!("{} {}", i64::MIN, u8::MAX);
}
