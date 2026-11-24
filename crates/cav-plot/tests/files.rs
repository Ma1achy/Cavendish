//! A drawing written as SVG and as PDF: the files are well formed and say what size they are.

use std::sync::Arc;

use cav_plot::drawing::{Anchor, Colour, Drawing, Face, Item};
use cav_plot::{pdf, svg};

/// A drawing with one of everything, and text that needs escaping.
fn everything() -> Drawing {
    let ink = Colour::hex(0x11_1111);
    let mut drawing = Drawing::new([240.944_9, 170.078_74], "a <test> & \"its\" ΔΦ");
    drawing.items = vec![
        Item::Rect {
            min: [0.0, 0.0],
            max: [240.944_9, 170.078_74],
            fill: Some(Colour::hex(0xff_ffff)),
            stroke: None,
        },
        Item::Rect { min: [30.0, 20.0], max: [220.0, 140.0], fill: None, stroke: Some((0.5, ink)) },
        Item::Clip { min: [30.0, 20.0], max: [220.0, 140.0] },
        Item::Line {
            points: vec![[30.0, 100.0], [80.0, 40.0], [150.0, 120.0], [260.0, 60.0]],
            width: 0.8,
            colour: Colour::hex(0x0b_0dff),
            dash: vec![4.0, 2.0],
        },
        Item::Circle { centre: [80.0, 40.0], radius: 1.5, fill: Colour::hex(0xe7_6f11) },
        Item::Diamond { centre: [150.0, 120.0], radius: 3.0, fill: ink, stroke: (0.5, ink) },
        Item::Unclip,
        Item::Image {
            min: [200.0, 4.0],
            max: [212.0, 12.0],
            width: 3,
            height: 2,
            rgb: Arc::new(vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9, 99, 99, 99, 250, 250, 250]),
        },
        Item::Text {
            at: [8.0, 4.0],
            anchor: Anchor::LEFT_TOP,
            text: "√S of ΔΦ <at> \"tower 2\" & f₀".into(),
            face: Face::Sans,
            size: 8.0,
            colour: ink,
            turned: false,
        },
        Item::Text {
            at: [125.0, 150.0],
            anchor: Anchor::CENTRE_TOP,
            text: "frequency, Hz".into(),
            face: Face::Mono,
            size: 7.0,
            colour: ink,
            turned: false,
        },
    ];
    drawing
}

#[test]
fn svg_and_pdf_are_well_formed() {
    let drawing = everything();
    // The SVG: balanced tags, escaped text, its size in millimetres and its viewBox in points.
    let text = svg::write(&drawing).unwrap();
    let checked = svg::check(&text).unwrap_or_else(|why| panic!("{why}\n{text}"));
    assert!((checked.size_mm[0] - 85.0).abs() < 0.01 && (checked.size_mm[1] - 60.0).abs() < 0.01);
    assert_eq!(checked.view_box, [0.0, 0.0, 240.945, 170.079]);
    assert_eq!(checked.labels, ["√S of ΔΦ <at> \"tower 2\" & f₀", "frequency, Hz"]);
    assert!(text.contains("<title>a &lt;test&gt; &amp; &quot;its&quot; ΔΦ</title>"));
    assert!(text.contains("stroke-dasharray=\"4 2\"") && text.contains("data:image/png;base64,iVBOR"));
    // One element each for: svg, title, 2 rects, clipPath and its rect, g, polyline, circle,
    // the diamond, the image, 2 texts.
    assert_eq!(checked.elements, 13);
    // Text is outlines: no <text> element, no font named, and each label's path has curves.
    assert!(!text.contains("<text") && !text.contains("font"));
    let label = text.lines().find(|line| line.contains("aria-label=\"frequency")).unwrap();
    assert!(label.matches('C').count() > 20 && label.matches('Z').count() >= 12, "{label}");
    // The same drawing is the same bytes.
    assert_eq!(svg::write(&drawing).unwrap(), text);

    // The PDF: the table's offsets point at the objects they name, each stream is as long as
    // it says, the file ends with %%EOF, and the page is the drawing's size.
    let bytes = pdf::write(&drawing);
    let checked = pdf::check(&bytes).unwrap();
    assert_eq!(checked.media_box, [0.0, 0.0, 240.945, 170.079]);
    assert_eq!(checked.objects, 6, "catalogue, pages, page, content, information and one picture");
    assert!(bytes.ends_with(b"%%EOF\n") && bytes.starts_with(b"%PDF-1.4\n"));
    // The content is paths: the y axis turned once, a clip, a dash, a filled and stroked
    // diamond, glyphs filled as curves; and no font or text operator anywhere.
    let content = &checked.content;
    assert!(content.starts_with("q\n1 0 0 -1 0 170.079 cm\n"), "{}", &content[..60]);
    for operator in ["\nW n\n", "\n[4 2] 0 d\n", "\nB\n", "\nf\n", "\nS\n", " re\n", " c\n", "/Im1 Do\n"] {
        assert!(content.contains(operator), "the stream has no `{}`", operator.trim());
    }
    let whole = String::from_utf8_lossy(&bytes);
    assert!(!whole.contains("/Font") && !content.contains("BT") && !content.contains(" Tj"));
    assert!(
        !whole.contains("/CreationDate") && !whole.contains("/ID"),
        "nothing that changes from run to run"
    );
    assert_eq!(content.matches("\nq\n").count() + 1, content.matches("Q\n").count(), "every q has its Q");
    assert_eq!(pdf::write(&drawing), bytes);

    // Control arms: each fault is found.
    // An offset in the table that is one byte out.
    let first = |what: &[u8], from: usize| {
        bytes[from..].windows(what.len()).position(|window| window == what).unwrap() + from
    };
    let table = first(b"\nxref\n", 0);
    let entry = first(b" 00000 n ", table) - 1;
    let mut broken = bytes.clone();
    broken[entry] = if broken[entry] == b'9' { b'8' } else { broken[entry] + 1 };
    let why = pdf::check(&broken).unwrap_err();
    assert!(why.contains("the table says object 1 is at byte") && why.contains("it is not"), "{why}");
    // A stream that is not as long as it says.
    let at = first(b"/Length ", 0) + 8;
    let mut broken = bytes.clone();
    broken[at] = if broken[at] == b'9' { b'8' } else { broken[at] + 1 };
    assert!(pdf::check(&broken).unwrap_err().contains("stream is not"));
    // A startxref that points elsewhere, and a file cut short.
    let start = first(b"startxref\n", 0) + 10;
    let mut broken = bytes.clone();
    broken[start] = if broken[start] == b'9' { b'8' } else { broken[start] + 1 };
    assert!(pdf::check(&broken).unwrap_err().contains("no `xref` there"));
    assert!(pdf::check(&bytes[..bytes.len() - 3]).unwrap_err().contains("%%EOF"));
    // An SVG with a tag left open, with a bare ampersand, with a bare `<` in an attribute.
    assert!(svg::check(&text.replacen("</g>\n", "", 1)).unwrap_err().contains("closes `<g>`"));
    let bare = svg::check(&text.replacen("&amp;", "&", 1)).unwrap_err();
    assert!(bare.contains("entity"), "{bare}");
    assert!(svg::check(&text.replacen("&lt;at", "<at", 1)).is_err());
    assert!(svg::check(&text.replace("mm\"", "\"")).unwrap_err().contains("millimetres"));
}
