fn main() {
    let text = std::fs::read_to_string("strs.txt").unwrap();
    let mut bad = 0;
    for line in text.lines() {
        let (s, b) = line.split_once(' ').unwrap();
        if s.parse::<f64>().unwrap().to_bits() != u64::from_str_radix(b, 16).unwrap() { bad += 1; }
    }
    println!("std f64::from_str vs CPython float(): {bad}/{} differ", text.lines().count());
}
