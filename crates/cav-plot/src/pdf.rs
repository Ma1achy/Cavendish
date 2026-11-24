//! A [`Drawing`] written as PDF, with no dependency: PDF 1.4, one page whose `MediaBox` is the
//! drawing's size in points, one uncompressed content stream of path operators, a
//! cross-reference table and a trailer.
//!
//! Text is the outlines of its glyphs, filled: the file embeds no font. There is no creation
//! date and no document identifier, so the same drawing is the same bytes. The page's y axis is
//! turned once (`1 0 0 -1 0 H cm`), after which the stream's coordinates are the drawing's own.

use std::fmt::Write as _;

use crate::drawing::{Colour, Drawing, Item};
use crate::fonts::Segment;
use crate::svg::number;

/// A quarter circle as a cubic Bézier: how far along the tangent each control point stands,
/// as a fraction of the radius.
const KAPPA: f32 = 0.552_284_8;

fn rgb(colour: Colour, operator: &str) -> String {
    let [r, g, b] = colour.0.map(|c| number(f32::from(c) / 255.0));
    format!("{r} {g} {b} {operator}\n")
}

fn xy(point: [f32; 2]) -> String {
    format!("{} {}", number(point[0]), number(point[1]))
}

/// The drawing's content stream, and the pictures it places (each as width, height and RGB bytes).
/// A picture a page places: its width and height in pixels, and its RGB bytes.
type Picture<'a> = (u32, u32, &'a [u8]);

fn content(drawing: &Drawing) -> (String, Vec<Picture<'_>>) {
    let mut out = String::new();
    let mut images = Vec::new();
    let _ = writeln!(out, "q\n1 0 0 -1 0 {} cm\n1 j", number(drawing.size[1]));
    for item in &drawing.items {
        match item {
            Item::Line { points, width, colour, dash } => {
                let Some((first, rest)) = points.split_first() else { continue };
                out.push_str(&rgb(*colour, "RG"));
                let dashes: Vec<String> = dash.iter().map(|d| number(*d)).collect();
                let _ = writeln!(out, "{} w\n[{}] 0 d", number(*width), dashes.join(" "));
                let _ = writeln!(out, "{} m", xy(*first));
                for point in rest {
                    let _ = writeln!(out, "{} l", xy(*point));
                }
                out.push_str("S\n");
            }
            Item::Rect { min, max, fill, stroke } => {
                let rect = format!("{} {} re", xy(*min), xy([max[0] - min[0], max[1] - min[1]]));
                if let Some(fill) = fill {
                    out.push_str(&rgb(*fill, "rg"));
                    let _ = writeln!(out, "{rect}\nf");
                }
                if let Some((width, colour)) = stroke {
                    out.push_str(&rgb(*colour, "RG"));
                    let _ = writeln!(out, "{} w\n[] 0 d\n{rect}\nS", number(*width));
                }
            }
            Item::Circle { centre: [x, y], radius: r, fill } => {
                let k = r * KAPPA;
                out.push_str(&rgb(*fill, "rg"));
                let _ = writeln!(out, "{} m", xy([x + r, *y]));
                for (a, b, to) in [
                    ([x + r, y + k], [x + k, y + r], [*x, y + r]),
                    ([x - k, y + r], [x - r, y + k], [x - r, *y]),
                    ([x - r, y - k], [x - k, y - r], [*x, y - r]),
                    ([x + k, y - r], [x + r, y - k], [x + r, *y]),
                ] {
                    let _ = writeln!(out, "{} {} {} c", xy(a), xy(b), xy(to));
                }
                out.push_str("h\nf\n");
            }
            Item::Diamond { centre: [x, y], radius: r, fill, stroke } => {
                out.push_str(&rgb(*fill, "rg"));
                out.push_str(&rgb(stroke.1, "RG"));
                let _ = writeln!(out, "{} w\n[] 0 d", number(stroke.0));
                let _ = writeln!(
                    out,
                    "{} m\n{} l\n{} l\n{} l\nh\nB",
                    xy([*x, y - r]),
                    xy([x + r, *y]),
                    xy([*x, y + r]),
                    xy([x - r, *y])
                );
            }
            Item::Text { at, anchor, text, face, size, colour, turned } => {
                let outline = crate::fonts::outline(text, *face, *size);
                if outline.segments.is_empty() {
                    continue;
                }
                let q = |point: &[f32; 2]| {
                    xy(crate::drawing::text_point(*at, *anchor, outline.size, *turned, *point))
                };
                out.push_str(&rgb(*colour, "rg"));
                for segment in &outline.segments {
                    match segment {
                        Segment::Move(to) => {
                            let _ = writeln!(out, "{} m", q(to));
                        }
                        Segment::Line(to) => {
                            let _ = writeln!(out, "{} l", q(to));
                        }
                        Segment::Cubic(a, b, to) => {
                            let _ = writeln!(out, "{} {} {} c", q(a), q(b), q(to));
                        }
                        Segment::Close => out.push_str("h\n"),
                    }
                }
                out.push_str("f\n");
            }
            Item::Image { min, max, width, height, rgb } => {
                images.push((*width, *height, rgb.as_slice()));
                // An image fills the unit square, its top at y = 1; the page's y is turned.
                let _ = writeln!(
                    out,
                    "q\n{} 0 0 {} {} cm\n/Im{} Do\nQ",
                    number(max[0] - min[0]),
                    number(min[1] - max[1]),
                    xy([min[0], max[1]]),
                    images.len()
                );
            }
            Item::Clip { min, max } => {
                let _ = writeln!(out, "q\n{} {} re\nW n", xy(*min), xy([max[0] - min[0], max[1] - min[1]]));
            }
            Item::Unclip => out.push_str("Q\n"),
        }
    }
    out.push_str("Q\n");
    (out, images)
}

/// A text as a PDF string that any reader takes: UTF-16, big-endian, in hexadecimal.
fn text_string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out.push('>');
    out
}

/// The drawing as a PDF file.
#[must_use]
pub fn write(drawing: &Drawing) -> Vec<u8> {
    let (stream, images) = content(drawing);
    let pictures: Vec<String> = (0..images.len()).map(|n| format!("/Im{} {} 0 R", n + 1, n + 6)).collect();
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /XObject << {} >> >> >>",
            number(drawing.size[0]),
            number(drawing.size[1]),
            pictures.join(" ")
        )
        .into_bytes(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()).into_bytes(),
        format!("<< /Title {} /Producer (Cavendish cav-plot) >>", text_string(&drawing.title)).into_bytes(),
    ];
    for (width, height, rgb) in images {
        let mut object = format!(
            "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",
            rgb.len()
        )
        .into_bytes();
        object.extend_from_slice(rgb);
        object.extend_from_slice(b"\nendstream");
        objects.push(object);
    }
    // The second line is a comment of four bytes above 127: it tells a reader the file is binary.
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (n, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", n + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let table = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{table}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// What [`check`] found a PDF to say of itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    /// The page's `MediaBox`: left, bottom, right, top, points.
    pub media_box: [f64; 4],
    /// How many objects the cross-reference table lists (the free entry not counted).
    pub objects: usize,
    /// The page's content stream.
    pub content: String,
}

fn find(bytes: &[u8], what: &[u8], from: usize) -> Option<usize> {
    bytes.get(from..)?.windows(what.len()).position(|window| window == what).map(|at| at + from)
}

/// Checks the structure of a PDF of the kind [`write`] makes: the header, `%%EOF` at the end,
/// `startxref` pointing at the cross-reference table, every entry of the table pointing at the
/// object it numbers, every stream as long as its `/Length` says, and a page with a `MediaBox`.
///
/// # Errors
/// What is wrong.
pub fn check(bytes: &[u8]) -> Result<Checked, String> {
    if !bytes.starts_with(b"%PDF-1.4\n") {
        return Err("no `%PDF-1.4` header".into());
    }
    let tail = bytes.strip_suffix(b"%%EOF\n").ok_or("the file does not end with `%%EOF`")?;
    let start = tail.windows(10).rposition(|window| window == b"startxref\n").ok_or("no `startxref`")?;
    let table: usize = std::str::from_utf8(&tail[start + 10..])
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .ok_or("`startxref` is not followed by an offset")?;
    let after = bytes.get(table..).filter(|rest| rest.starts_with(b"xref\n")).ok_or_else(|| {
        format!("`startxref` says the table is at byte {table}, and there is no `xref` there")
    })?;
    let text = String::from_utf8_lossy(&after[..after.len().min(start - table)]).into_owned();
    let mut lines = text.lines().skip(1);
    let count: usize = lines
        .next()
        .and_then(|line| line.strip_prefix("0 "))
        .and_then(|count| count.parse().ok())
        .ok_or("the table does not say how many entries it has")?;
    if lines.next() != Some("0000000000 65535 f ") {
        return Err("the table's first entry is not the free one".into());
    }
    for number in 1..count {
        let line = lines.next().ok_or_else(|| format!("the table has no entry for object {number}"))?;
        let offset: usize = line
            .strip_suffix(" 00000 n ")
            .filter(|digits| digits.len() == 10)
            .and_then(|digits| digits.parse().ok())
            .ok_or_else(|| format!("`{line}` is not an entry of the table"))?;
        let head = format!("{number} 0 obj\n");
        if !bytes.get(offset..).is_some_and(|rest| rest.starts_with(head.as_bytes())) {
            return Err(format!("the table says object {number} is at byte {offset}, and it is not"));
        }
        // A stream is as long as its dictionary says.
        let end = find(bytes, b"endobj", offset).ok_or_else(|| format!("object {number} never ends"))?;
        if let Some(stream) = find(&bytes[..end], b">>\nstream\n", offset) {
            let dictionary = String::from_utf8_lossy(&bytes[offset..stream]);
            let length: usize = dictionary
                .rsplit_once("/Length ")
                .and_then(|(_, rest)| rest.trim().parse().ok())
                .ok_or_else(|| format!("object {number} is a stream with no /Length"))?;
            let from = stream + 10;
            if bytes.get(from + length..from + length + 10) != Some(b"\nendstream") {
                return Err(format!("object {number}'s stream is not {length} bytes long"));
            }
        }
    }
    let whole = String::from_utf8_lossy(bytes);
    let media: Vec<f64> = whole
        .split_once("/MediaBox [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(numbers, _)| numbers.split(' ').filter_map(|n| n.parse().ok()).collect())
        .ok_or("no page with a MediaBox")?;
    let media_box: [f64; 4] = media.try_into().map_err(|_| "the MediaBox is not four numbers")?;
    let content = whole
        .split_once("4 0 obj\n")
        .and_then(|(_, rest)| rest.split_once("stream\n"))
        .and_then(|(_, rest)| rest.split_once("\nendstream"))
        .map(|(content, _)| content.to_owned())
        .ok_or("no content stream")?;
    Ok(Checked { media_box, objects: count - 1, content })
}
