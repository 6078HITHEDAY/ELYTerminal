//! Renders an SVG into a macOS iconset: `icons SVG FOLDER`.

use std::path::Path;

use anyhow::Context as _;
use resvg::{tiny_skia, usvg};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let svg = args.next().context("icons: name an SVG to render")?;
    let out = args
        .next()
        .context("icons: name an iconset folder to write")?;
    let data = std::fs::read(&svg).with_context(|| format!("icons: cannot read {svg}"))?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default())
        .with_context(|| format!("icons: {svg} is not an SVG"))?;
    std::fs::create_dir_all(&out).with_context(|| format!("icons: cannot make {out}"))?;
    for points in [16, 32, 128, 256, 512] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let side = points * scale;
            let mut pixmap = tiny_skia::Pixmap::new(side, side).context("icons: no pixmap")?;
            let fit = side as f32 / tree.size().width();
            let transform = tiny_skia::Transform::from_scale(fit, fit);
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            let png = Path::new(&out).join(format!("icon_{points}x{points}{suffix}.png"));
            pixmap
                .save_png(&png)
                .with_context(|| format!("icons: cannot write {}", png.display()))?;
        }
    }
    println!("icons: {svg} drawn into {out}");
    Ok(())
}
