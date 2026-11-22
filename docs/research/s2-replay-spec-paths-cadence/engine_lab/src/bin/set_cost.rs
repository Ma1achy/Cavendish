//! What one `Command::Set` costs as the scene grows, and whether a NaN can reach the journal.
use cav_engine::{Command, Engine};
use cav_sim::{BodySpec, SceneSpec};
use std::time::Instant;

fn big(n: usize) -> SceneSpec {
    let mut s = SceneSpec::room();
    for i in 0..n {
        s.bodies.push(BodySpec::point(&format!("b{i}"), 1.0 + i as f64, [i as f64 * 0.01, 3.0, 1.0]));
    }
    s
}

fn main() {
    for n in [1usize, 100, 1000] {
        let spec = big(n);
        let json = serde_json::to_string(&spec).unwrap();
        let reps = 200;
        let t = Instant::now();
        for _ in 0..reps {
            let v = serde_json::to_value(&spec).unwrap();
            let _: SceneSpec = serde_json::from_value(v).unwrap();
        }
        let round_trip = t.elapsed().as_secs_f64() / f64::from(reps);
        let t = Instant::now();
        for _ in 0..reps {
            let _ = spec.clone();
        }
        let clone = t.elapsed().as_secs_f64() / f64::from(reps);
        let t = Instant::now();
        for _ in 0..reps {
            spec.validate().unwrap();
        }
        let validate = t.elapsed().as_secs_f64() / f64::from(reps);
        let mut e = Engine::new();
        e.apply(Command::load(spec)).unwrap();
        e.settle(std::time::Duration::from_secs(20));
        let t = Instant::now();
        for i in 0..reps {
            let path = format!("bodies[{}].mass", n / 2);
            e.apply(Command::Set { path, value: serde_json::json!(5.0 + f64::from(i)) }).unwrap();
        }
        let set = t.elapsed().as_secs_f64() / f64::from(reps);
        println!(
            "{n:>5} bodies: {:>7} bytes of JSON; to_value + from_value {:.0} µs; clone {:.1} µs; validate {:.1} µs; Engine::apply(Set) {:.0} µs",
            json.len(),
            round_trip * 1e6,
            clone * 1e6,
            validate * 1e6,
            set * 1e6
        );
    }
    let mut e = Engine::new();
    println!("Seek(NaN): {:?}", e.apply(Command::Seek(f64::NAN)));
    println!("journal after: {}", serde_json::to_string(e.journal()).unwrap());
    println!("console `seek nan`: {:?}", e.run_line("seek nan"));
    println!("Set duration null: {:?}", e.apply(Command::Set { path: "duration".into(), value: serde_json::json!(null) }));
    println!("JSON text 1e999: {:?}", serde_json::from_str::<serde_json::Value>("1e999").map_err(|e| e.to_string()));
    println!("Set duration inf (written as null by JSON): {:?}", e.apply(Command::Set { path: "duration".into(), value: serde_json::json!(f64::INFINITY) }));
}
