//! The paper drawing: a plan or a section of the scene as a technical drawing (S4 brief, R16 and
//! bound row W11).

use std::path::{Path, PathBuf};

use cav_engine::{Engine, PaperView};
use cav_plot::drawing::{Drawing, Item};
use cav_plot::fonts::Outlines;
use cav_plot::paper::{
    Drawn, MAX_PAPER_BRICKS, MAX_PAPER_ITEMS, MAX_PAPER_POINTS, Revision, drawing, drawing_within,
};
use cav_plot::style::{MM_PER_POINT, Style};
use cav_plot::{figure, pdf, svg};
use cav_sim::SceneSpec;
use cav_sim::dev::{stress_1e10, world_box};

fn revision() -> Revision {
    Revision { version: "0.0.0".into(), revision: "0123456789abcdef".into(), dirty: Some(false) }
}

fn points(preset: &str) -> (Style, [f32; 2]) {
    let style = Style::preset(preset).unwrap();
    let points = [(style.size_mm[0] / MM_PER_POINT) as f32, (style.size_mm[1] / MM_PER_POINT) as f32];
    (style, points)
}

fn draw(scene: &SceneSpec, view: PaperView, preset: &str) -> (Drawing, Drawn) {
    let (style, size) = points(preset);
    drawing(scene, 0.0, view, &style, size, true, &revision(), &Outlines)
}

/// A world-box whose world holds nothing.
fn empty_world() -> SceneSpec {
    let mut scene = world_box();
    scene.world.as_mut().unwrap().pieces.clear();
    scene
}

/// The parts R16 asks for, each as a reason it is missing.
fn missing_parts(drawing: &Drawing, drawn: &Drawn) -> Vec<&'static str> {
    let texts = drawing.texts();
    let mut missing = Vec::new();
    if drawn.title_block.len() < 5 || !drawn.title_block.iter().all(|line| texts.contains(&line.as_str())) {
        missing.push("title block");
    }
    if !drawn.title_block.iter().any(|line| line.starts_with("Scale 1:")) {
        missing.push("scale in the title block");
    }
    let bar_label = cav_plot::paper::length_text(drawn.scale_bar);
    if !(drawn.scale_bar > 0.0 && texts.contains(&bar_label.as_str()) && texts.contains(&"0")) {
        missing.push("scale bar");
    }
    if !texts.contains(&"N") {
        missing.push("north arrow");
    }
    if drawn.dimensions.is_empty()
        || !drawn.dimensions.iter().all(|(label, _)| texts.contains(&label.as_str()))
    {
        missing.push("dimension line");
    }
    if svg::check(&svg::write(drawing).unwrap()).is_err() || pdf::check(&pdf::write(drawing)).is_err() {
        missing.push("well-formed SVG and PDF");
    }
    missing
}

#[test]
fn a_paper_drawing_has_its_parts() {
    // A section through the box: every part, and hatching.
    let (section, drawn) = draw(&world_box(), PaperView::SectionY(7.0), "paper-full");
    assert_eq!(missing_parts(&section, &drawn), Vec::<&str>::new(), "{:?}", drawn.title_block);
    assert!(!drawn.hatch_items.is_empty() && !drawn.outline_items.is_empty(), "{drawn:?}");
    assert!(drawn.hatch_items.clone().all(|n| matches!(section.items[n], Item::Line { .. })));
    assert_eq!(drawn.north, "into the page");
    // The world's extent along x is dimensioned at its true length.
    assert!(drawn.dimensions.iter().any(|(label, m)| label == "4 m" && *m == 4.0), "{:?}", drawn.dimensions);
    // A plan: its parts, the north arrow pointing up, outlines and no hatching.
    let (plan, drawn) = draw(&world_box(), PaperView::Plan, "paper-column");
    assert_eq!(missing_parts(&plan, &drawn), Vec::<&str>::new());
    assert!(drawn.hatch_items.is_empty() && !drawn.outline_items.is_empty());
    assert_eq!(drawn.north, "arrow up");
    // The towers' spacing is dimensioned (the room's towers stand along x).
    assert!(drawn.dimensions.len() >= 3, "{:?}", drawn.dimensions);
    // Control: a section of a world with no matter has no hatching (and no outline), and a
    // drawing with its title block taken out is seen to miss it.
    let (empty, drawn) = draw(&empty_world(), PaperView::SectionY(7.0), "paper-full");
    assert!(drawn.hatch_items.is_empty() && drawn.outline_items.is_empty(), "{drawn:?}");
    assert!(empty.texts().contains(&"The world holds no matter"));
    let (mut cut, drawn) = draw(&world_box(), PaperView::SectionY(7.0), "paper-full");
    cut.items.retain(|item| !matches!(item, Item::Text { text, .. } if text == &drawn.title_block[2]));
    assert!(missing_parts(&cut, &drawn).contains(&"title block"));
    // Control: a section that misses the world hatches nothing, and says so.
    let (_, drawn) = draw(&world_box(), PaperView::SectionY(-30.0), "paper-full");
    assert!(drawn.hatch_items.is_empty());
    assert!(drawn.notes.iter().any(|n| n.contains("the cut misses the world")), "{:?}", drawn.notes);
}

/// The box round every outline point, in the scene's metres `(u, v)`.
fn outline_box(drawing: &Drawing, drawn: &Drawn) -> [f64; 4] {
    let mut b = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
    for n in drawn.outline_items.clone() {
        let Item::Line { points, .. } = &drawing.items[n] else { panic!("an outline is a line") };
        for p in points {
            let u = drawn.centre[0] + (f64::from(p[0]) - drawn.centre_page[0]) / drawn.points_per_metre;
            let v = drawn.centre[1] - (f64::from(p[1]) - drawn.centre_page[1]) / drawn.points_per_metre;
            b = [b[0].min(u), b[1].max(u), b[2].min(v), b[3].max(v)];
        }
    }
    b
}

fn within_a_cell(found: [f64; 4], expected: [f64; 4], cell: f64) -> bool {
    found.iter().zip(expected).all(|(a, b)| (a - b).abs() <= cell)
}

#[test]
fn the_plan_of_world_box_outlines_the_box_at_its_true_size() {
    // The box is 6 to 10 m east and 5 to 9 m north.
    let truth = [6.0, 10.0, 5.0, 9.0];
    for preset in ["paper-column", "paper-full", "poster"] {
        let (plan, drawn) = draw(&world_box(), PaperView::Plan, preset);
        let cell = drawn.cell.unwrap();
        let found = outline_box(&plan, &drawn);
        assert!(within_a_cell(found, truth, cell), "{preset}: {found:?} at cells of {cell} m");
        // The cells drawn are about a millimetre on paper, or coarser.
        assert!(cell * 1000.0 / drawn.scale >= 0.999, "{preset}: {cell} m at 1:{}", drawn.scale);
    }
    // Control: the box made a metre longer is not drawn at the old size.
    let mut longer = world_box();
    if let cav_sim::world::PieceKind::Box { size, centre } =
        &mut longer.world.as_mut().unwrap().pieces[0].kind
    {
        size[0] += 1.0;
        centre[0] += 0.5;
    }
    let (plan, drawn) = draw(&longer, PaperView::Plan, "paper-full");
    assert!(!within_a_cell(outline_box(&plan, &drawn), truth, drawn.cell.unwrap()));
}

#[test]
fn slow_a_paper_drawing_is_bounded() {
    // About two seconds in `quick`: five drawings of a kilometre, each from up to
    // `MAX_PAPER_BRICKS` bricks; the bound is on what a big world makes, so a big world it is.
    // The kilometre of stress-1e10, plan and section, on the largest preset: within the caps,
    // drawn from coarse bricks.
    for view in [PaperView::Plan, PaperView::SectionY(-395.0)] {
        let (picture, drawn) = draw(&stress_1e10(), view, "poster");
        assert!(picture.items.len() <= MAX_PAPER_ITEMS, "{view:?}: {} items", picture.items.len());
        assert!(drawn.bricks <= MAX_PAPER_BRICKS && drawn.bricks > 0, "{view:?}: {} bricks", drawn.bricks);
        assert!(!drawn.outline_items.is_empty(), "{view:?}");
    }
    // The caps at work, on a world made busy: stress-1e10's kilometre with its ground made rough
    // (30 m of relief at a 7 m wavelength), on a sheet a metre square. At a millimetre a cell it
    // is past the brick cap, so it is drawn coarser, and says so; it is within the item and point
    // caps there.
    let mut rough = stress_1e10();
    for piece in &mut rough.world.as_mut().unwrap().pieces {
        if let cav_sim::world::PieceKind::Terrain { relief, wavelength, .. } = &mut piece.kind {
            (*relief, *wavelength) = (30.0, 7.0);
        }
    }
    let style = Style::preset("poster").unwrap();
    let size = [(1000.0 / MM_PER_POINT) as f32; 2];
    let within = |cap: usize| {
        drawing_within(&rough, 0.0, PaperView::Plan, &style, size, true, &revision(), &Outlines, cap)
    };
    let (held, drawn) = within(MAX_PAPER_ITEMS);
    assert!(held.items.len() <= MAX_PAPER_ITEMS, "{} items", held.items.len());
    assert!(points_in(&held) <= MAX_PAPER_POINTS, "{} points", points_in(&held));
    assert!(drawn.bricks <= MAX_PAPER_BRICKS && drawn.level > drawn.finest_level, "{drawn:?}");
    assert!(drawn.notes.iter().any(|n| n.contains("in place of")), "{:?}", drawn.notes);
    // The item cap: held to a tenth of what that drawing holds, the same plan is drawn coarser
    // and is within it. Control: the drawing it would otherwise be is past it.
    let cap = held.items.len() / 10;
    let (fewer, fewer_drawn) = within(cap);
    assert!(held.items.len() > cap);
    assert!(fewer.items.len() <= cap, "{} items, cap {cap}", fewer.items.len());
    assert!(fewer_drawn.level > drawn.level, "{:?} then {:?}", drawn.level, fewer_drawn.level);
    // A cap with no room for the world at all cuts its lines, and says so.
    let frame = held.items.len() - drawn.outline_items.len() - drawn.hatch_items.len();
    let (cut, drawn) = within(frame);
    assert!(cut.items.len() <= frame, "{} items, cap {frame}", cut.items.len());
    assert!(drawn.notes.iter().any(|n| n.contains("left out")), "{:?}", drawn.notes);
}

/// The points of every line of a drawing.
fn points_in(drawing: &Drawing) -> usize {
    drawing.items.iter().map(|item| if let Item::Line { points, .. } = item { points.len() } else { 0 }).sum()
}

#[test]
fn a_scene_without_a_world_draws_its_towers_and_says_so() {
    let room = SceneSpec::room();
    let (plan, drawn) = draw(&room, PaperView::Plan, "paper-full");
    let texts = plan.texts();
    assert!(texts.contains(&"No world: towers and bodies only"), "{texts:?}");
    assert!(drawn.notes.iter().any(|n| n.contains("no world")));
    assert!(drawn.outline_items.is_empty() && drawn.level.is_none());
    for tower in &room.array.towers {
        assert!(texts.contains(&tower.label.as_str()), "{} not labelled", tower.label);
    }
    let rings = plan
        .items
        .iter()
        .filter(|i| matches!(i, Item::Circle { fill, .. } if *fill == cav_plot::paper::INK))
        .count();
    assert_eq!(rings, room.array.towers.len());
    // Control: the world-box (the same towers and a world) says nothing of the kind.
    let (plan, _) = draw(&world_box(), PaperView::Plan, "paper-full");
    assert!(!plan.texts().contains(&"No world: towers and bodies only"));
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/paper").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn word(path: &Path) -> String {
    format!("\"{}\"", path.display().to_string().replace('\\', "/"))
}

#[test]
fn a_paper_figure_regenerates_bit_for_bit_from_its_command() {
    let write = |dir: &Path| {
        let mut engine = Engine::new();
        figure::install(&mut engine);
        engine.run_line("load world-box").unwrap();
        for (view, file) in [("plan", "plan.svg"), ("section y 7", "cut.svg"), ("section x 8", "cut-x.pdf")] {
            engine
                .run_line(&format!("figure paper {view} as paper-full to {}", word(&dir.join(file))))
                .unwrap();
        }
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        (read("plan.svg"), read("cut.svg"), read("cut-x.pdf"), read("plan.json"))
    };
    let (a, b) = (write(&scratch("one")), write(&scratch("two")));
    assert!(a == b, "the same commands wrote different bytes");
    // The record says what was drawn and the command that draws it again.
    let record: serde_json::Value = serde_json::from_slice(&a.3).unwrap();
    let entry = &record["files"][0];
    assert_eq!(entry["paper"]["view"], "plan");
    assert!(entry["paper"]["scale"].as_str().unwrap().starts_with("1:"));
    assert!(entry["command"]["console"].as_str().unwrap().starts_with("figure paper plan as paper-full"));
    assert!(record["data"].is_null());
    // Control: the plan and the section are different drawings.
    assert_ne!(a.0, a.1);
}
