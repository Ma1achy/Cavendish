use cav_core::forward::*;
use cav_core::Vec3;
use std::sync::Arc;

fn src(m: f64, at: Vec3) -> Mass<impl Fn(f64) -> Vec3 + Sync> { Mass::point(m, move |_| at) }

fn p1(t: f64) -> Vec3 { Vec3::new(10.0 + 0.3*t, 2.0, 3.0 + (0.5*t).sin()) }
fn p2(t: f64) -> Vec3 { Vec3::new(-9.0, 4.0*(0.2*t).cos(), 1.0) }
fn mk() -> [Mass<fn(f64)->Vec3>; 2] { [Mass::point(10.0, p1 as fn(f64)->Vec3), Mass::point(70.0, p2 as fn(f64)->Vec3)] }
fn main() {
    let aion = Instrument::aion10();
    let aice = Instrument::aice();
    for (n, i) in [("AION-10", aion), ("AICE", aice)] {
        let k_eff = std::f64::consts::TAU * i.kicks / i.wavelength;
        println!("{n}: k_eff={:.4e} rad/m  v_kick={:.4} m/s  dr={} T={} u0={:.4} g={}", k_eff, i.kick_velocity(), i.ifo_separation, i.half_sequence, i.launch_velocity, i.gravity);
        // arms
        let [l, u] = i.arms();
        let top = (0..=2000).map(|s| u.rise(i.flight_time()*s as f64/2000.0)).fold(0.0, f64::max);
        println!("   upper arm apex above its launch point = {top:.2} m; landings lower {:?} upper {:?}", l.landings(), u.landings());
        let rd = Reader::new(i);
        let fl = rd.flight();
        for (mass, d, h) in [(10.0, 10.0, 1.0), (1000.0, 10.0, 1.0), (10.0, 10.0, 0.5*i.ifo_separation), (10.0, 100.0, 1.0), (10.0, 100.0, 0.5*i.ifo_separation), (10.0, 1000.0, 1.0)] {
            let s = src(mass, Vec3::new(d, 0.0, h));
            let eng = rd.phase(Vec3::ZERO, &s, 50.0);
            // first-order-in-arm-separation ("quasi-static field") estimate: -k_eff * sum w * dz * a_z(z_mid)
            let az = |z: f64| i.big_g * mass * (h - z) / (d*d + (h-z)*(h-z)).powf(1.5);
            let mut ph = [0.0f64; 2];
            for n in &fl.nodes { for (k, hh) in [0.0, i.ifo_separation].into_iter().enumerate() {
                let dz = n.upper - n.lower; let mid = hh + 0.5*(n.upper + n.lower);
                ph[k] += n.weight * dz * (-az(mid)); } }
            let qs1 = k_eff * (ph[0] - ph[1]) * 1.0; // k_eff = m v_kick/hbar *... see note: (m/hbar) integral dz a = k_eff/v_kick * ...
            let qs1 = qs1 / i.kick_velocity();
            // pure point-gradient estimate k_eff * Gamma * dr * T^2, Gamma = d a_z/dz at the baseline midpoint
            let z0 = 0.5*i.ifo_separation; let e = 1e-3;
            let gam = (az(z0+e) - az(z0-e))/(2.0*e);
            let qs2 = k_eff * gam * i.ifo_separation * i.half_sequence.powi(2);
            println!("   M={mass:>5} kg lateral D={d:>6} m h={h:5.2}: engine {eng:+.4e}  field@mid-arm {qs1:+.4e} (x{:.3})  k*Gam*dr*T^2 {qs2:+.4e} (x{:.3})", qs1/eng, qs2/eng);
        }
    }
    for (n, i) in [("AION-10", aion), ("AICE", aice)] {
        let rd = Reader::new(i);
        for (d, h) in [(10.0, 1.0), (3.0, 1.0), (10.0, 35.0)] {
            let a = rd.read_to(Vec3::ZERO, &src(10.0, Vec3::new(d, 0.0, h)), 50.0, PHASE_TOLERANCE);
            let fixed = rd.read(Vec3::ZERO, &src(10.0, Vec3::new(d, 0.0, h)), 50.0);
            println!("audit {n} D={d} h={h}: halvings {} estimate {:.2e} rad; fixed-rule vs audited ΔΦ diff {:.2e} rad; nodes {}", a.halvings, a.estimate.unwrap(), (fixed.gradiometer(i.sign)-a.reading.gradiometer(i.sign)).abs(), rd.flight().nodes.len());
        }
    }
    // bit-equality: unequal towers, shared reader vs own reader vs threads in other order
    let a = Instrument::aion10(); let b = Instrument::aice();
    let towers = [(a, Vec3::new(0.0,0.0,0.0)), (b, Vec3::new(30.0,5.0,0.0)), (a, Vec3::new(-4.0,0.0,0.0)), (b, Vec3::new(30.0,-60.0,0.0))];
    let bodies = mk();
    let alone: Vec<f64> = towers.iter().map(|(i, p)| { let r = Reader::new(*i); bodies.iter().map(|m| r.phase(*p, m, 60.0)).sum::<f64>() }).collect();
    // shared readers, reverse order, in parallel threads
    let ra = Arc::new(Reader::new(a)); let rb = Arc::new(Reader::new(b));
    let mut hs = vec![];
    for (k, (i, p)) in towers.iter().enumerate().rev() {
        let r = if *i == a { ra.clone() } else { rb.clone() };
        let p = *p;
        hs.push(std::thread::spawn(move || {
            let bodies = mk();
            (k, bodies.iter().map(|m| r.phase(p, m, 60.0)).sum::<f64>())
        }));
    }
    let mut shared = vec![0.0; 4];
    for h in hs { let (k, v) = h.join().unwrap(); shared[k] = v; }
    for k in 0..4 { println!("tower {k}: alone {:+.6e} shared/threads {:+.6e} bits equal: {}", alone[k], shared[k], alone[k].to_bits()==shared[k].to_bits()); }
    // control: change tower 2's instrument (T by 1e-9 relative) -> only tower 2 changes
    let a2 = Instrument { half_sequence: a.half_sequence*(1.0+1e-9), ..a };
    let r2 = Reader::new(a2);
    let t2: f64 = bodies.iter().map(|m| r2.phase(towers[2].1, m, 60.0)).sum();
    println!("control: tower2 with T*(1+1e-9): {:+.6e} vs {:+.6e}, bits equal {}", t2, alone[2], t2.to_bits()==alone[2].to_bits());
    println!("tower0 unchanged (same inst): {}", alone[0].to_bits()==shared[0].to_bits());
    // two readers of equal instruments: identical bits?
    let r_a = Reader::new(a); let r_b = Reader::new(a);
    let x = r_a.phase(towers[0].1, &bodies[0], 60.0); let y = r_b.phase(towers[0].1, &bodies[0], 60.0);
    println!("two readers of one instrument: bits equal {}", x.to_bits()==y.to_bits());
}
