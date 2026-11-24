//! Colour maps for fields drawn on voxel bodies: sequential (one hue, by lightness) for a size,
//! diverging (two hues about a grey middle) for a signed value. Both are drawn from the
//! `dataviz` skill's reference ramps (`references/palette.md`: the blue sequential steps; blue and
//! red about a neutral grey), with the changes each constant's note gives (S3: the least must
//! clear 2:1 against the surface; each arm must darken, or lighten, steadily under every
//! simulated colour-blindness), so that a field's colours and its legend read the same in the 3-D view,
//! the application and a figure. `cav-plot` holds them to colour-blind vision and grey print
//! (`the_colour_maps_survive_colour_blindness_and_print`). On a dark surface the sequential ramp
//! runs the other way and the diverging arms are turned round ([`DIVERGING_DARK`]).

/// The sequential ramp on a light surface, least to greatest, sRGB: the reference palette's blue,
/// steps 300, 400, 550 and 700. It starts at step 300, not 100, because leaves are discrete marks
/// (the reference's ordinal rule): the least must clear 2:1 against white and against the paper
/// style's cream (`#f1ede4`: 2.14:1), so the finest level, the commonest, does not vanish (the
/// `dataviz` validator, `--ordinal`, failed step 100 at 1.29:1 and step 250 at 1.81:1 on cream);
/// four steps, because steps 250 to 400 are closer than its 0.06 of lightness apart.
pub const SEQUENTIAL: [u32; 4] = [0x6da7ec, 0x3987e5, 0x1c5cab, 0x0d366b];

/// The sequential ramp on a dark surface, least to greatest: steps 600 to 100, so that the least
/// clears 2:1 against the dark background (step 700 was 1.66:1).
pub const SEQUENTIAL_DARK: [u32; 6] = [0x184f95, 0x256abf, 0x3987e5, 0x6da7ec, 0x9ec5f4, 0xcde2fb];

/// The diverging ramp from most negative to most positive, sRGB: blue, the reference's neutral
/// grey (`#f0efec`), red. The steps next to the middle are darker than the reference's lightest
/// (`#6da7ec`, not `#9ec5f4`): with the pale one Brettel's tritanopia sees the first step towards
/// blue as lighter than the grey (L* 91.7 to 92.5), and a lighter grey did not cure it (94.1 to
/// 94.6 with `#f7f6f3`, 96.1 to 96.3 with `#fcfcfb`); a step down of 30 L* does
/// (`cav-plot`'s colour-map test, S3).
pub const DIVERGING: [u32; 7] = [0x0d366b, 0x2a78d6, 0x6da7ec, 0xf0efec, 0xec8a78, 0xc8402e, 0x7a1a10];

/// The diverging ramp on a dark surface, most negative to most positive: two arms that grow
/// lighter away from a dark grey middle (`#383835`), as the light ramp grows darker away from a
/// light one, so a value further from zero stands further from the surface either way. The steps
/// next to the middle are well above it (`#1c5cab`, `#b8392a`): with `#8a1f14` Machado's
/// protanopia saw the red arm start darker than the grey.
pub const DIVERGING_DARK: [u32; 7] = [0xb7d3f6, 0x5598e7, 0x1c5cab, 0x383835, 0xb8392a, 0xe57a68, 0xf6b9ad];

fn channels(hex: u32) -> [f64; 3] {
    [f64::from((hex >> 16) & 0xff), f64::from((hex >> 8) & 0xff), f64::from(hex & 0xff)]
}

/// The colour `t` (0 to 1) of the way along `stops`, interpolated in sRGB: `[r, g, b]` bytes.
fn along(stops: &[u32], t: f64) -> [u8; 3] {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let place = t * (stops.len() - 1) as f64;
    let i = (place.floor() as usize).min(stops.len() - 2);
    let f = place - i as f64;
    let (a, b) = (channels(stops[i]), channels(stops[i + 1]));
    std::array::from_fn(|c| (a[c] + (b[c] - a[c]) * f).round() as u8)
}

/// A size on the sequential ramp: `t` from 0 (the least drawn) to 1 (the greatest). On a dark
/// surface the ramp runs from dark to light, so that the greatest stands furthest from the
/// background either way.
#[must_use]
pub fn sequential(t: f64, dark: bool) -> [u8; 3] {
    if dark { along(&SEQUENTIAL_DARK, t) } else { along(&SEQUENTIAL, t) }
}

/// A signed value on the diverging ramp: `t` from −1 (the most negative drawn) to 1.
#[must_use]
pub fn diverging(t: f64, dark: bool) -> [u8; 3] {
    along(if dark { &DIVERGING_DARK } else { &DIVERGING }, 0.5 * (t + 1.0))
}

/// The colour of `value` within `(least, greatest)`: the diverging ramp about zero when
/// `diverging`, else the sequential ramp. A range of one value is its middle.
#[must_use]
pub fn of_value(value: f64, (least, greatest): (f64, f64), diverging_map: bool, dark: bool) -> [u8; 3] {
    if diverging_map {
        let m = least.abs().max(greatest.abs());
        diverging(if m > 0.0 { value / m } else { 0.0 }, dark)
    } else {
        let span = greatest - least;
        sequential(if span > 0.0 { (value - least) / span } else { 0.5 }, dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maps_run_from_end_to_end() {
        assert_eq!(sequential(0.0, false), [0x6d, 0xa7, 0xec]);
        assert_eq!(sequential(1.0, false), [0x0d, 0x36, 0x6b]);
        assert_eq!((sequential(0.0, true), sequential(1.0, true)), ([0x18, 0x4f, 0x95], [0xcd, 0xe2, 0xfb]));
        assert_eq!(diverging(0.0, false), [0xf0, 0xef, 0xec]);
        assert_eq!(diverging(0.0, true), [0x38, 0x38, 0x35]);
        assert_eq!(of_value(-2.0, (-2.0, 1.0), true, false), diverging(-1.0, false));
        assert_eq!(of_value(f64::NAN, (0.0, 1.0), false, false), sequential(0.0, false));
        // Control: the two ends of the diverging map are not the same hue.
        assert_ne!(diverging(-1.0, false), diverging(1.0, false));
    }
}
