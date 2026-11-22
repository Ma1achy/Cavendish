//! Signal sizes and the sensitivity of a nearby mass's phase to local g, on the engine's own model.
use cav_core::forward::{Instrument, Mass, Reader};
use cav_core::Vec3;
fn read(i: Instrument, m: f64, p: Vec3) -> f64 {
    let r = Reader::new(i);
    let s = Mass::point(m, move |_t| p);
    r.phase(Vec3::ZERO, &s, 2.0 * i.half_sequence)
}
fn main() {
    let a = Instrument::aion10();
    println!("kick {:.4} m/s ; symmetric launch AION {:.4}", a.kick_velocity(), a.symmetric_launch());
    for (d, z) in [(5.0, 5.0), (10.0, 5.0), (10.0, 0.0)] {
        println!("AION 1 kg at d={d} z={z}: {:.6e} rad", read(a, 1.0, Vec3::new(d, 0.0, z)));
    }
    let c = Instrument::aice();
    println!("AICE launch {:.4}; 1kg d=10 z=35: {:.6e}", c.launch_velocity, read(c, 1.0, Vec3::new(10.0, 0.0, 35.0)));
    let p = Vec3::new(5.0, 0.0, 5.0);
    let base = read(a, 1.0, p);
    println!("g sweep, u0 fixed 3.86:");
    for g in [9.78, 9.7803253, 9.80665, 9.81, 9.8322, 9.7, 9.9] {
        let mut i = a; i.gravity = g;
        let ph = read(i, 1.0, p);
        println!("  g={g:.4}  {ph:.6e}  rel {:+.3e}", ph / base - 1.0);
    }
    println!("g sweep, launch closed (gT - kick/2):");
    for g in [9.78, 9.81, 9.8322] {
        let mut i = a; i.gravity = g; i.launch_velocity = i.symmetric_launch();
        let ph = read(i, 1.0, p);
        println!("  g={g:.4} u0={:.4} {ph:.6e}  rel {:+.3e}", i.launch_velocity, ph / base - 1.0);
    }
    for (name, g) in [("Moon", 1.62), ("Mars", 3.71)] {
        let mut i = a; i.gravity = g;
        println!("{name} g={g}: AION T=0.73 u0 fixed 3.86: {:.6e}; symmetric u0 = {:.3} (negative = impossible)", read(i, 1.0, p), i.symmetric_launch());
        let tmin = 0.5 * i.kick_velocity() / g;
        let mut j = i; j.half_sequence = tmin * 1.2; j.launch_velocity = j.symmetric_launch();
        println!("  closed launch needs T > {:.3} s ; at T={:.3}: u0={:.3} phase {:.6e}", tmin, j.half_sequence, j.launch_velocity, read(j, 1.0, p));
    }
}
