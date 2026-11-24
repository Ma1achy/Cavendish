//! The engine's numbers in a browser build equal the native ones (`docs/slices/S17.md` §2).
//!
//! One function, [`numbers`], drives the engine as a client does (load a dev scene, settle, ask
//! for the signal, the reading under a playhead and a periodogram) and lists every number it
//! got back. `cargo xtask web` runs it twice:
//!
//! 1. natively, as the ignored test `web_equivalence_native_numbers`, which writes the numbers'
//!    bit patterns to the file `CAV_WEB_EQUIVALENCE` names;
//! 2. compiled for `wasm32-unknown-unknown` with that file built in, under wasm-bindgen's test
//!    runner in Node.js (`wasm_equals_native`), which works the numbers out again there and holds
//!    them to the native ones.
//!
//! The wasm run is the web build's engine: its forward stage runs from `poll` with no threads
//! (`forward/inline.rs`), its clock is `web_time`, its maths is the wasm target's. It is not the
//! browser's WebGPU, which computes nothing yet (S5 moves gravity to the GPU; that needs a
//! browser-driven test then).
//!
//! Natively, outside `cargo xtask web`, the ignored test is not run, so `quick` pays nothing.

use std::time::Duration;

use cav_engine::{ChannelName, Command, Engine, Query, Reply};
use cav_sim::{SceneSpec, dev};

/// How long the engine may take to settle a scene, natively or in Node.js.
const PATIENCE: Duration = Duration::from_secs(600);

/// The scenes compared: a point mass, a voxel sphere (the cube kernel) and an oscillator's comb
/// (the Lomb–Scargle or FFT periodogram).
fn scenes() -> Vec<SceneSpec> {
    vec![dev::point(), dev::sphere(), dev::comb()]
}

/// Every number the engine gives a client for each scene, labelled `scene/what`.
fn numbers() -> Vec<(String, Vec<f64>)> {
    let mut out = Vec::new();
    for scene in scenes() {
        let name = scene.name.clone();
        let mut engine = Engine::new();
        engine.apply(Command::load(scene)).expect("a dev scene loads");
        engine.apply(Command::Seek(1.0)).expect("a seek");
        assert!(engine.settle(PATIENCE), "{name}: not settled in {PATIENCE:?}");
        let (_, signal) = engine.signal();
        let signal = signal.expect("a settled scene has a signal");
        out.push((format!("{name}/signal.lower"), signal.lower.clone()));
        out.push((format!("{name}/signal.upper"), signal.upper.clone()));
        out.push((format!("{name}/signal.gradiometer"), signal.gradiometer.clone()));
        out.push((format!("{name}/signal.clock"), signal.clock.clone()));
        let Ok(Reply::Reading(reading)) = engine.query(&Query::Reading(Some(1.0))) else {
            panic!("{name}: no reading")
        };
        let phases = reading.towers.iter().flat_map(|t| [t.lower, t.upper, t.gradiometer]).collect();
        out.push((format!("{name}/reading@1s"), phases));
        let asked = Query::Periodogram { tower: 1, channel: ChannelName::Gradiometer, compared: false };
        let Ok(Reply::Periodogram(periodogram)) = engine.query(&asked) else {
            panic!("{name}: no periodogram")
        };
        out.push((format!("{name}/periodogram.frequencies"), periodogram.frequencies));
        out.push((format!("{name}/periodogram.power"), periodogram.power));
    }
    out
}

/// The numbers as text: one line `label index bits` a number, bits in hexadecimal.
fn as_text(numbers: &[(String, Vec<f64>)]) -> String {
    let mut text = String::new();
    for (label, values) in numbers {
        for (index, value) in values.iter().enumerate() {
            text.push_str(&format!("{label} {index} {:016x}\n", value.to_bits()));
        }
    }
    text
}

/// What a comparison found: how many numbers, how many to the bit, which labels differ, and how
/// far (relative to the largest magnitude under the label), split by whether the label's
/// arithmetic goes through the maths library ([`THROUGH_LIBM`]).
#[derive(Debug, Default, PartialEq)]
struct Found {
    numbers: usize,
    same_bits: usize,
    /// The largest difference under a label of [`THROUGH_LIBM`].
    worst: f64,
    worst_at: String,
    /// Each label with numbers that differ in their bits, and how many.
    differing: Vec<(String, usize)>,
    /// The first number that differs under a label *not* in [`THROUGH_LIBM`]: where the
    /// arithmetic is the same, one bit is a failure.
    changed_bit: Option<String>,
    faults: Vec<String>,
}

impl Found {
    /// Whether the comparison holds: no fault, no bit changed outside [`THROUGH_LIBM`], and the
    /// labels in it within [`TOLERANCE`].
    fn holds(&self) -> bool {
        self.faults.is_empty() && self.changed_bit.is_none() && self.worst <= TOLERANCE
    }
}

/// The labels whose numbers go through `sin`, `cos`, `exp` or `log`, and so may differ in their
/// last bits (wasm takes Rust's own maths library, native the platform's): `comb`'s oscillator
/// is placed by a sine (its three signal channels, and the reading under the playhead), and its
/// periodogram's power goes through the FFT's twiddles. Every other label is held to the bit. A
/// label is added here only with a line in the slice notes saying which function it goes through.
const THROUGH_LIBM: [&str; 5] = [
    "comb/signal.lower",
    "comb/signal.upper",
    "comb/signal.gradiometer",
    "comb/reading@1s",
    "comb/periodogram.power",
];

/// Holds `ours` to `theirs` (as [`as_text`] writes them).
fn compare(ours: &[(String, Vec<f64>)], theirs: &str) -> Found {
    let mut found = Found::default();
    let mut lines = theirs.lines();
    for (label, values) in ours {
        let scale = values.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let through_libm = THROUGH_LIBM.contains(&label.as_str());
        for (index, value) in values.iter().enumerate() {
            found.numbers += 1;
            let Some(line) = lines.next() else {
                found.faults.push(format!("{label} {index}: the native list has ended"));
                return found;
            };
            let mut parts = line.split(' ');
            let (Some(their_label), Some(their_index), Some(bits)) =
                (parts.next(), parts.next(), parts.next())
            else {
                found.faults.push(format!("`{line}` is not `label index bits`"));
                return found;
            };
            if their_label != label || their_index != index.to_string() {
                found
                    .faults
                    .push(format!("{label} {index}: the native list has {their_label} {their_index}"));
                return found;
            }
            let theirs = f64::from_bits(u64::from_str_radix(bits, 16).unwrap_or(u64::MAX));
            if theirs.to_bits() == value.to_bits() {
                found.same_bits += 1;
                continue;
            }
            match found.differing.last_mut() {
                Some((last, count)) if last == label => *count += 1,
                _ => found.differing.push((label.clone(), 1)),
            }
            if !through_libm {
                found.changed_bit.get_or_insert_with(|| format!("{label} {index}: {value:e} vs {theirs:e}"));
                continue;
            }
            let relative = if scale > 0.0 { (theirs - value).abs() / scale } else { f64::INFINITY };
            // A NaN on one side only is as far apart as numbers can be.
            let relative = if relative.is_nan() { f64::INFINITY } else { relative };
            if relative > found.worst {
                (found.worst, found.worst_at) =
                    (relative, format!("{label} {index}: {value:e} vs {theirs:e}"));
            }
        }
    }
    if lines.next().is_some() {
        found.faults.push("the native list has more numbers than this one".into());
    }
    found
}

/// The tolerance under a label of [`THROUGH_LIBM`]: a difference relative to the largest number
/// under the same label. wasm and x86-64 both round every operation as IEEE 754 says and neither
/// fuses a multiply and an add here, so elsewhere the arithmetic is the same and is held to the
/// bit; a last-bit difference out of `sin` may grow through a sum or an FFT. 1e-12 of the largest
/// phase is far below a microradian.
const TOLERANCE: f64 = 1e-12;

#[test]
fn the_comparison_finds_a_changed_bit() {
    // Control arm for the wasm test. A list compared with itself holds, every bit the same.
    let ours = vec![
        ("a/x".to_owned(), vec![1.0, -2.5, 3.25e-9]),
        ("comb/periodogram.power".to_owned(), vec![0.0, 7.0]),
    ];
    let same = compare(&ours, &as_text(&ours));
    assert_eq!((same.numbers, same.same_bits, same.worst, same.faults.len()), (5, 5, 0.0, 0));
    assert!(same.holds());
    // One number moved by its last bit under a label whose arithmetic is the same: found, named,
    // and the comparison FAILS.
    let mut moved = ours.clone();
    moved[0].1[1] = f64::from_bits((-2.5_f64).to_bits() + 1);
    let found = compare(&ours, &as_text(&moved));
    assert_eq!(found.same_bits, 4);
    assert_eq!(found.differing, vec![("a/x".to_owned(), 1)]);
    assert!(found.changed_bit.as_deref().is_some_and(|at| at.starts_with("a/x 1:")), "{found:?}");
    assert!(!found.holds(), "a last-bit change outside THROUGH_LIBM must fail: {found:?}");
    // The same last bit under a label of THROUGH_LIBM is within the tolerance and holds; a part
    // in a thousand there does not.
    let mut moved = ours.clone();
    moved[1].1[1] = f64::from_bits(7.0_f64.to_bits() + 1);
    let found = compare(&ours, &as_text(&moved));
    assert!(found.changed_bit.is_none() && found.worst > 0.0 && found.worst < 1e-15, "{found:?}");
    assert!(found.worst_at.starts_with("comb/periodogram.power 1:"), "{found:?}");
    assert!(found.holds());
    moved[1].1[1] = 7.0 * (1.0 + 1e-3);
    assert!(!compare(&ours, &as_text(&moved)).holds());
    // A shorter list is a fault.
    let shorter = vec![ours[0].clone()];
    assert!(!compare(&ours, &as_text(&shorter)).holds());
}

/// Natively: writes the numbers for the wasm run. Ignored outside `cargo xtask web`.
#[test]
#[ignore = "run by `cargo xtask web`, which names the file to write in CAV_WEB_EQUIVALENCE"]
fn web_equivalence_native_numbers() {
    let path = std::env::var("CAV_WEB_EQUIVALENCE").expect("CAV_WEB_EQUIVALENCE names the file to write");
    let numbers = numbers();
    std::fs::write(&path, as_text(&numbers)).expect("the numbers are written");
    let count: usize = numbers.iter().map(|(_, values)| values.len()).sum();
    println!("web equivalence: {count} native numbers written to {path}");
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::{compare, numbers};

    /// The native numbers, built in when `cargo xtask web` compiles this.
    const NATIVE: &str = include_str!(env!("CAV_WEB_EQUIVALENCE"));

    #[wasm_bindgen_test]
    fn wasm_equals_native() {
        let found = compare(&numbers(), NATIVE);
        // The line `cargo xtask web` records.
        wasm_bindgen_test::console_log!(
            "web equivalence: {} numbers, {} the same to the bit; outside THROUGH_LIBM: {}; under it, largest relative difference {:e}{}; differing: {:?}",
            found.numbers,
            found.same_bits,
            found
                .changed_bit
                .as_ref()
                .map_or_else(|| "every bit the same".to_owned(), |at| format!("a bit CHANGED at {at}")),
            found.worst,
            if found.worst_at.is_empty() { String::new() } else { format!(" at {}", found.worst_at) },
            found.differing
        );
        assert!(found.numbers > 1000, "too few numbers to say anything: {}", found.numbers);
        assert!(found.holds(), "{found:?}");
    }
}
