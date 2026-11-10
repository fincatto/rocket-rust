use std::collections::HashMap;

fn main() {
    // HashMap usa o RNG do sistema (ProcessPrng no std moderno).
    let mut m = HashMap::new();
    m.insert("x", 1);
    std::thread::spawn(|| ()).join().unwrap();
    println!("{m:?}");
}
