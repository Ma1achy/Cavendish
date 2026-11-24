//! Colour maps for fields drawn on voxel bodies: sequential (one hue, by lightness) for a size,
//! diverging (two hues about a grey middle) for a signed value. Both are the `dataviz` skill's
//! reference ramps (`references/palette.md`: the blue sequential steps 100 to 700; blue and red
//! about a neutral grey), so that a field's colours and its legend read the same in the 3-D view,
//! the application and a figure. `cav-plot` holds them to colour-blind vision and grey print
//! (`the_colour_maps_survive_colour_blindness_and_print`).

/// The sequential ramp, light to dark, sRGB: the reference palette's blue, steps 100 to 700.
pub const SEQUENTIAL: [u32; 7] = [0xcde2fb, 0x9ec5f4, 0x6da7ec, 0x3987e5, 0x256abf, 0x184f95, 0x0d366b];

/// The diverging ramp from most negative to most positive, sRGB: blue, a neutral grey, red. The
/// middle is the light surface's neutral (`#f0efec`); on a dark surface it is `#383835`.
pub const DIVERGING: [u32; 7] = [0x0d366b, 0x3987e5, 0x9ec5f4, 0xf0efec, 0xf2a99b, 0xd8503e, 0x8a1f14];

/// The diverging ramp's middle on a dark surface.
pub const DIVERGING_DARK_MIDDLE: u32 = 0x38_3835;

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
/// surface the ramp runs the other way, so that the least recedes into the background either way.
#[must_use]
pub fn sequential(t: f64, dark: bool) -> [u8; 3] {
    along(&SEQUENTIAL, if dark { 1.0 - t } else { t })
}

/// A signed value on the diverging ramp: `t` from −1 (the most negative drawn) to 1.
#[must_use]
pub fn diverging(t: f64, dark: bool) -> [u8; 3] {
    let mut stops = DIVERGING;
    if dark {
        stops[3] = DIVERGING_DARK_MIDDLE;
    }
    along(&stops, 0.5 * (t + 1.0))
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
        assert_eq!(sequential(0.0, false), [0xcd, 0xe2, 0xfb]);
        assert_eq!(sequential(1.0, false), [0x0d, 0x36, 0x6b]);
        assert_eq!(sequential(0.0, true), sequential(1.0, false), "dark runs the other way");
        assert_eq!(diverging(0.0, false), [0xf0, 0xef, 0xec]);
        assert_eq!(diverging(0.0, true), [0x38, 0x38, 0x35]);
        assert_eq!(of_value(-2.0, (-2.0, 1.0), true, false), diverging(-1.0, false));
        assert_eq!(of_value(f64::NAN, (0.0, 1.0), false, false), sequential(0.0, false));
        // Control: the two ends of the diverging map are not the same hue.
        assert_ne!(diverging(-1.0, false), diverging(1.0, false));
    }
}
