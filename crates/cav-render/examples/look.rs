//! Renders a library scene to a PNG: `cargo run -p cav-render --example look -- <scene> <view> <style> <out.png>`.
//! A developer's peephole; `cav render` is the real tool.

use cav_render::{Frame, Renderer};
use cav_sim::{DrawStyle, NamedView, Projection, ViewSpec};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [scene, view, style, out] = args.as_slice() else {
        return Err("usage: look <scene> <view> <style> <out.png>".into());
    };
    let scene = cav_sim::load(scene).map_err(|e| e.0)?;
    let named = NamedView::parse(view).ok_or("unknown view")?;
    let style = DrawStyle::parse(style).ok_or("unknown style")?;
    let projection = if named == NamedView::Iso { Projection::Perspective } else { Projection::Orthographic };
    let view = ViewSpec::framing(&scene, named, projection, style);
    let gpu = cav_gpu::Gpu::new()?;
    let renderer = Renderer::new(&gpu);
    let frame = Frame { scene: &scene, view: &view, highlight: Some(cav_sim::Target::Tower(2)) };
    let png = renderer.render_png(&gpu, frame, [1200, 800])?;
    std::fs::write(out, png).map_err(|e| e.to_string())
}
