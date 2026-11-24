//! A [`Drawing`] written as SVG.
//!
//! The picture's width and height are stated in millimetres and its `viewBox` in points, so one
//! unit of the file is one point of the drawing. Text is written as the outlines of its glyphs
//! ([`crate::fonts::outline`]), each with the text itself as its `aria-label`: the file needs no
//! font and looks the same wherever it is opened. Nothing in it depends on the time or the
//! machine: the same drawing is the same bytes.

use std::fmt::Write as _;

use crate::drawing::{Colour, Drawing, Item};
use crate::fonts::Segment;
use crate::style::MM_PER_POINT;

/// A length or a coordinate as the files write it: to a thousandth, with no trailing zeros.
#[must_use]
pub fn number(value: f32) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" { "0".to_owned() } else { text.to_owned() }
}

/// Text made safe for an attribute or an element of XML.
#[must_use]
pub fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Bytes as base64 (RFC 4648, padded): for the picture inside an SVG.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = u32::from(chunk[0]) << 16
            | u32::from(chunk.get(1).copied().unwrap_or(0)) << 8
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for (k, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if k <= chunk.len() {
                out.push(ALPHABET[(word >> shift) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn dash_of(dash: &[f32]) -> String {
    if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{}\"", dash.iter().map(|d| number(*d)).collect::<Vec<_>>().join(" "))
    }
}

fn stroke_of(width: f32, colour: Colour) -> String {
    format!("stroke=\"{}\" stroke-width=\"{}\"", colour.css(), number(width))
}

/// The drawing as an SVG document.
///
/// # Errors
/// When a picture in it cannot be encoded.
pub fn write(drawing: &Drawing) -> Result<String, String> {
    let [w, h] = drawing.size;
    let mm = |points: f32| number((f64::from(points) * MM_PER_POINT) as f32);
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">",
        mm(w),
        mm(h),
        number(w),
        number(h)
    );
    let _ = writeln!(out, "<title>{}</title>", escaped(&drawing.title));
    let mut clips = 0usize;
    for item in &drawing.items {
        match item {
            Item::Line { points, width, colour, dash } => {
                let list: Vec<String> =
                    points.iter().map(|q| format!("{},{}", number(q[0]), number(q[1]))).collect();
                let _ = writeln!(
                    out,
                    "<polyline fill=\"none\" {} stroke-linejoin=\"round\"{} points=\"{}\"/>",
                    stroke_of(*width, *colour),
                    dash_of(dash),
                    list.join(" ")
                );
            }
            Item::Rect { min, max, fill, stroke } => {
                let _ = writeln!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
                    number(min[0]),
                    number(min[1]),
                    number(max[0] - min[0]),
                    number(max[1] - min[1]),
                    fill.map_or_else(|| "none".to_owned(), Colour::css),
                    stroke
                        .map_or_else(String::new, |(width, colour)| format!(" {}", stroke_of(width, colour))),
                );
            }
            Item::Polygon { points, fill } => {
                let list: Vec<String> =
                    points.iter().map(|q| format!("{},{}", number(q[0]), number(q[1]))).collect();
                let _ = writeln!(out, "<polygon fill=\"{}\" points=\"{}\"/>", fill.css(), list.join(" "));
            }
            Item::Circle { centre, radius, fill } => {
                let _ = writeln!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                    number(centre[0]),
                    number(centre[1]),
                    number(*radius),
                    fill.css()
                );
            }
            Item::Diamond { centre: [x, y], radius: r, fill, stroke } => {
                let _ = writeln!(
                    out,
                    "<path d=\"M{} {}L{} {}L{} {}L{} {}Z\" fill=\"{}\" {}/>",
                    number(*x),
                    number(y - r),
                    number(x + r),
                    number(*y),
                    number(*x),
                    number(y + r),
                    number(x - r),
                    number(*y),
                    fill.css(),
                    stroke_of(stroke.0, stroke.1)
                );
            }
            Item::Text { at, anchor, text, face, size, colour, turned } => {
                let outline = crate::fonts::outline(text, *face, *size);
                let mut d = String::new();
                let q = |point: &[f32; 2]| {
                    let [x, y] = crate::drawing::text_point(*at, *anchor, outline.size, *turned, *point);
                    format!("{} {}", number(x), number(y))
                };
                for segment in &outline.segments {
                    match segment {
                        Segment::Move(to) => {
                            let _ = write!(d, "M{}", q(to));
                        }
                        Segment::Line(to) => {
                            let _ = write!(d, "L{}", q(to));
                        }
                        Segment::Cubic(a, b, to) => {
                            let _ = write!(d, "C{} {} {}", q(a), q(b), q(to));
                        }
                        Segment::Close => d.push('Z'),
                    }
                }
                let _ = writeln!(
                    out,
                    "<path aria-label=\"{}\" fill=\"{}\" d=\"{d}\"/>",
                    escaped(text),
                    colour.css()
                );
            }
            Item::Image { min, max, width, height, rgb } => {
                let rgba: Vec<u8> = rgb.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
                let png = cav_render::encode_png(&rgba, [*width, *height])?;
                let _ = writeln!(
                    out,
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{}\"/>",
                    number(min[0]),
                    number(min[1]),
                    number(max[0] - min[0]),
                    number(max[1] - min[1]),
                    base64(&png)
                );
            }
            Item::Clip { min, max } => {
                clips += 1;
                let _ = writeln!(
                    out,
                    "<clipPath id=\"clip{clips}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>\n<g clip-path=\"url(#clip{clips})\">",
                    number(min[0]),
                    number(min[1]),
                    number(max[0] - min[0]),
                    number(max[1] - min[1]),
                );
            }
            Item::Unclip => out.push_str("</g>\n"),
        }
    }
    out.push_str("</svg>\n");
    Ok(out)
}

/// What [`check`] found an SVG to say of itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    /// The stated width and height, millimetres.
    pub size_mm: [f64; 2],
    /// The `viewBox`: x, y, width, height, in the file's units (points).
    pub view_box: [f64; 4],
    /// How many elements the document holds.
    pub elements: usize,
    /// Every `aria-label`: the texts written as outlines, unescaped, in order.
    pub labels: Vec<String>,
}

fn unescaped(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let end = rest[at..].find(';').ok_or("an `&` that starts no entity")? + at;
        out.push(match &rest[at + 1..end] {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            other => return Err(format!("`&{other};` is not an entity this checker knows")),
        });
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Checks that `text` is well-formed XML of the kind [`write`] makes (elements with quoted
/// attributes, text, no comments or CDATA), that every tag opened is closed in order, that text
/// and attributes hold no bare `<` or `&`, and that the root is an `svg` with its size in
/// millimetres and a `viewBox`.
///
/// # Errors
/// What is wrong, and where.
pub fn check(text: &str) -> Result<Checked, String> {
    let body = text.strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>").ok_or("no XML declaration")?;
    let mut open: Vec<&str> = Vec::new();
    let (mut elements, mut labels, mut root) = (0usize, Vec::new(), None);
    let mut rest = body;
    while let Some(at) = rest.find('<') {
        unescaped(&rest[..at]).map_err(|why| format!("in text: {why}"))?;
        let end = rest[at..].find('>').ok_or("a `<` that is never closed")? + at;
        let tag = &rest[at + 1..end];
        rest = &rest[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            let opened = open.pop().ok_or_else(|| format!("`</{name}>` closes nothing"))?;
            if opened != name {
                return Err(format!("`</{name}>` closes `<{opened}>`"));
            }
            continue;
        }
        let (tag, empty) = tag.strip_suffix('/').map_or((tag, false), |tag| (tag, true));
        let (name, mut attributes) = tag.split_once(' ').unwrap_or((tag, ""));
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("`<{name}` is not an element's name"));
        }
        if open.is_empty() && root.is_some() {
            return Err(format!("`<{name}>` after the root element has closed"));
        }
        elements += 1;
        let mut found = Vec::new();
        while !attributes.trim().is_empty() {
            let (key, after) = attributes
                .trim_start()
                .split_once("=\"")
                .ok_or_else(|| format!("`{attributes}` in `<{name}>` is not an attribute"))?;
            let (value, after) =
                after.split_once('"').ok_or_else(|| format!("`{key}` in `<{name}>` has no closing quote"))?;
            if value.contains('<') {
                return Err(format!("a bare `<` in `{key}` of `<{name}>`"));
            }
            found.push((key, unescaped(value).map_err(|why| format!("in `{key}` of `<{name}>`: {why}"))?));
            attributes = after;
        }
        if let Some((_, label)) = found.iter().find(|(key, _)| *key == "aria-label") {
            labels.push(label.clone());
        }
        if open.is_empty() {
            if name != "svg" {
                return Err(format!("the root is `<{name}>`, not `<svg>`"));
            }
            root = Some(found);
        }
        if !empty {
            open.push(name);
        }
    }
    if let Some(left) = open.last() {
        return Err(format!("`<{left}>` is never closed"));
    }
    if !rest.trim().is_empty() {
        return Err("text after the root element".into());
    }
    let root = root.ok_or("no root element")?;
    let attribute = |key: &str| {
        root.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| format!("the svg has no `{key}`"))
    };
    let mm = |key: &str| -> Result<f64, String> {
        let value = attribute(key)?;
        value
            .strip_suffix("mm")
            .and_then(|number| number.parse().ok())
            .ok_or_else(|| format!("`{key}=\"{value}\"` is not a length in millimetres"))
    };
    let view: Vec<f64> = attribute("viewBox")?.split(' ').filter_map(|v| v.parse().ok()).collect();
    let view_box: [f64; 4] = view.try_into().map_err(|_| "the viewBox is not four numbers")?;
    Ok(Checked { size_mm: [mm("width")?, mm("height")?], view_box, elements, labels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_text_and_base64_are_written_plainly() {
        assert_eq!(number(240.944_9), "240.945");
        assert_eq!(number(85.0), "85");
        assert_eq!(number(-0.0001), "0");
        assert_eq!(number(0.5), "0.5");
        assert_eq!(escaped("a < b & \"c\" > d"), "a &lt; b &amp; &quot;c&quot; &gt; d");
        assert_eq!(unescaped(&escaped("a < b & \"c\" > d")).unwrap(), "a < b & \"c\" > d");
        // RFC 4648's own examples.
        for (plain, coded) in
            [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")]
        {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
    }
}
