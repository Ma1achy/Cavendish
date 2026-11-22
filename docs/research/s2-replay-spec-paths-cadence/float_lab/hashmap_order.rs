use std::collections::HashMap;
fn main() {
    let m: HashMap<u64, f64> = (0..8u64).map(|k| (k, k as f64 * 0.1)).collect();
    let keys: Vec<u64> = m.keys().copied().collect();
    let sum: f64 = m.values().sum();
    println!("iteration order {keys:?}; sum of values in that order {:016x}", sum.to_bits());
}
