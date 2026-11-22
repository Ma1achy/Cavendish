//! What the scene file does with numbers that are not ordinary.
use cav_sim::SceneSpec;
fn main() {
    let ron = SceneSpec::room().to_ron();
    let line = ron.lines().find(|l| l.trim_start().starts_with("duration:")).unwrap().to_string();
    for bad in ["1e400", "inf", "NaN", "-0.0", "10"] {
        let text = ron.replace(&line, &format!("    duration: {bad},"));
        println!("duration {bad:<6} -> {:?}", SceneSpec::from_ron(&text).map(|s| s.duration).map_err(|e| e.0));
    }
    let a = SceneSpec::room();
    let r1 = a.to_ron();
    println!("ron text stable under a round trip: {}", SceneSpec::from_ron(&r1).unwrap().to_ron() == r1);
    let j1 = a.to_json();
    println!("json text stable under a round trip: {}", SceneSpec::from_json(&j1).unwrap().to_json() == j1);
}
