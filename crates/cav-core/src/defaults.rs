//! The defaults every scene starts from: the AION-10 instrument and the five-tower array in its room.
//!
//! Sources: `plan/VALIDATION-AND-RESEARCH.md` §1 (the AION-10 preset), `reference/design/instrument.md` §3
//! (Δr = 5 m in a 10 m tower), George's answers of 23 Nov 2026 (the tower is 10.1 m) and the v2
//! recording behind the viewer mock-up (`docs/viewer-mockup/replay.json`: the tower layout and the
//! room). The array layout and the room are a default and no more: there is no real layout, this
//! is a prospective study (George, 23 Nov 2026; `plan/PROGRESS.md`, "For George").

/// Height of one tower (the baseline), metres. 10.1 and not the plan's 10 (George, 23 Nov 2026):
/// with ⁸⁷Sr the upper arm of the upper interferometer reaches 10.006 m. Nothing the instrument
/// reads depends on it: it is the tower that is drawn, and the room an arm must stay inside.
pub const TOWER_HEIGHT: f64 = 10.1;
/// Vertical separation of a tower's two interferometers (Δr), metres.
pub const IFO_SEPARATION: f64 = 5.0;
/// Half the interrogation sequence, T, seconds (2T = 1.46 s from launch to readout).
pub const HALF_SEQUENCE: f64 = 0.73;
/// Launch velocity u₀, metres per second.
pub const LAUNCH_VELOCITY: f64 = 3.86;
/// Time between shots of one tower, seconds (Nyquist 0.25 Hz).
pub const CYCLE: f64 = 2.0;

/// Ground positions of the five towers, each `[x, y]` (east, north) in metres in the world's
/// frame; the towers' bases are at z = 0. One at the centre and four on a cross with 4 m arms.
/// A default layout; there is no real one: this is a prospective study.
pub const TOWER_POSITIONS: [[f64; 2]; 5] = [[0.0, 0.0], [4.0, 0.0], [-4.0, 0.0], [0.0, 4.0], [0.0, -4.0]];

/// The room's minimum corner, `[x, y, z]` (east, north, up) in metres in the world's frame. Its
/// floor is the plane z = 0, where the towers stand.
pub const ROOM_ORIGIN: [f64; 3] = [-5.0, -5.0, 0.0];
/// The room's size, `[east–west, north–south, height]` in metres: 10 × 10 in plan, 3 high. The
/// towers rise through its ceiling.
pub const ROOM_SIZE: [f64; 3] = [10.0, 10.0, 3.0];

/// How long a new scene's timeline is, seconds of scene time from t = 0.
pub const DURATION: f64 = 12.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_array_is_five_towers_in_a_cross_inside_the_room() {
        assert_eq!(TOWER_POSITIONS.len(), 5);
        assert_eq!(TOWER_POSITIONS[0], [0.0, 0.0]);
        for p in &TOWER_POSITIONS[1..] {
            // Each arm tower is 4 m from the centre, on an axis.
            assert_eq!(p[0].hypot(p[1]), 4.0);
            assert!(p[0] == 0.0 || p[1] == 0.0);
        }
        // The cross is symmetric: the positions sum to zero.
        let sum = TOWER_POSITIONS.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
        assert_eq!(sum, [0.0, 0.0]);
        for p in TOWER_POSITIONS {
            for (axis, &c) in p.iter().enumerate() {
                assert!(c > ROOM_ORIGIN[axis] && c < ROOM_ORIGIN[axis] + ROOM_SIZE[axis]);
            }
        }
    }

    #[test]
    fn instrument_defaults_are_the_aion_10_preset() {
        // VALIDATION-AND-RESEARCH §1: 10 m baseline, Δr = 5 m, 2T = 1.46 s, 2 s cycle, Nyquist 0.25 Hz.
        // The numbers themselves, so that a change to any of them is a failed test and not a
        // quiet change of instrument (reference/design/instrument.md §3: Δr 5 m in a 10 m tower).
        // The tower is 10.1 m, not the plan's 10: George's answer of 23 Nov 2026, so that the
        // ⁸⁷Sr arms fit (`the_default_arms_top_out_below_the_tower` in `forward.rs`).
        assert_eq!(TOWER_HEIGHT, 10.1);
        assert_eq!(IFO_SEPARATION, 5.0);
        assert_eq!(HALF_SEQUENCE, 0.73);
        assert_eq!(LAUNCH_VELOCITY, 3.86);
        assert_eq!(CYCLE, 2.0);
        assert_eq!(2.0 * HALF_SEQUENCE, 1.46);
        assert_eq!(0.5 / CYCLE, 0.25);
        // The upper interferometer launches from Δr, and the tower stands a little over 2Δr.
        let over = TOWER_HEIGHT - 2.0 * IFO_SEPARATION;
        assert!(over > 0.0 && over < 0.5, "{over} m over twice the separation");
        // A shot must finish before the next one of the same tower starts.
        let flight = 2.0 * HALF_SEQUENCE;
        assert!(flight < CYCLE, "flight {flight} s must fit in the cycle {CYCLE} s");
    }
}
