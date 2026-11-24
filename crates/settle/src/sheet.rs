//! `sheet.png`: the 2D review sheet of a run, north up. Terrain tinted by height, water, the kept town (its land and
//! its footprints, dark, and its exits), blocks, lots by use, streets by rank at their own width, a scale bar and a
//! north mark. Colours and the label font are presentation
//! constants, not tunables; the scale comes from the recipe (`sheet_m_per_px`).
// Raster, pixel and metre conversions: values are bounded by the raster dims and the sheet size.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use crate::run::{CultureRun, Prepared};
use magna_urbis_core::urban::{Point, Shape};
use tiny_skia::{FillRule, IntSize, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

type Rgb = [u8; 3];
const LAND_LOW: Rgb = [214, 222, 196];
const LAND_HIGH: Rgb = [196, 180, 152];
const WATER: Rgb = [120, 170, 214];
const BLOCK: Rgb = [236, 230, 214];
/// The kept town: its land, its footprints and its exits.
const KEPT_LAND: Rgb = [104, 98, 92];
const KEPT_FOOTPRINT: Rgb = [34, 30, 28];
const KEPT_EXIT: Rgb = [60, 30, 30];
const LOT_EDGE: Rgb = [120, 110, 96];
/// Lot fill by Magna Urbis `use` (`types.rs:338` `Parcel.use_kind`); anything else gets `LOT_OTHER`.
const LOT_USES: [(&str, Rgb); 4] = [("plot", [232, 196, 160]), ("compound", [214, 170, 140]), ("garden", [190, 214, 160]), ("reserve", [200, 200, 200])];
const LOT_OTHER: Rgb = [220, 200, 210];
/// Street colour by rank (0 = main and regional, `geography.rs:564`); ranks past the end use the last.
const STREET_RANKS: [Rgb; 4] = [[150, 30, 30], [200, 110, 40], [80, 80, 80], [130, 130, 130]];
const INK: Rgb = [20, 20, 20];
const PAPER: Rgb = [255, 255, 255];
/// Thinnest stroke drawn (px), so minor lanes stay visible at coarse scales.
const MIN_STROKE_PX: f32 = 0.8;
/// Label glyphs, 3x5, rows top to bottom, bit 2 = left column.
const GLYPHS: [(char, [u8; 5]); 12] = [
    ('0', [7, 5, 5, 5, 7]),
    ('1', [2, 6, 2, 2, 7]),
    ('2', [7, 1, 7, 4, 7]),
    ('3', [7, 1, 3, 1, 7]),
    ('4', [5, 5, 7, 1, 1]),
    ('5', [7, 4, 7, 1, 7]),
    ('6', [7, 4, 7, 5, 7]),
    ('7', [7, 1, 1, 1, 1]),
    ('8', [7, 5, 7, 5, 7]),
    ('9', [7, 5, 7, 1, 7]),
    ('m', [0, 0, 7, 7, 5]),
    ('N', [5, 7, 7, 5, 5]),
];
/// Glyph pixel size (px) and margin (px) of the sheet furniture.
const GLYPH_PX: f32 = 3.0;
const MARGIN_PX: f32 = 12.0;

fn paint(rgb: Rgb, alpha: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(rgb[0], rgb[1], rgb[2], alpha);
    p.anti_alias = true;
    p
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |i: usize| (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t).round() as u8;
    [f(0), f(1), f(2)]
}

fn ring(pb: &mut PathBuilder, ring: &[Point], s: f32) {
    for (i, p) in ring.iter().enumerate() {
        let (x, y) = (p.x as f32 * s, p.y as f32 * s);
        if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
    }
    pb.close();
}

fn shape_path(shape: &Shape, s: f32) -> Option<tiny_skia::Path> {
    if shape.outer.len() < 3 {
        return None;
    }
    let mut pb = PathBuilder::new();
    ring(&mut pb, &shape.outer, s);
    for hole in &shape.holes {
        ring(&mut pb, hole, s);
    }
    pb.finish()
}

fn text(pix: &mut Pixmap, at: (f32, f32), label: &str) {
    let ink = paint(INK, 255);
    let mut x = at.0;
    for c in label.chars() {
        if let Some((_, rows)) = GLYPHS.iter().find(|(g, _)| *g == c) {
            for (r, bits) in rows.iter().enumerate() {
                for col in 0..3u8 {
                    if bits & (4 >> col) != 0
                        && let Some(rect) = Rect::from_xywh(x + f32::from(col) * GLYPH_PX, at.1 + r as f32 * GLYPH_PX, GLYPH_PX, GLYPH_PX)
                    {
                        pix.fill_rect(rect, &ink, Transform::identity(), None);
                    }
                }
            }
        }
        x += 4.0 * GLYPH_PX;
    }
}

/// The round length (m) of the scale bar: 1, 2 or 5 times a power of ten, at most a fifth of the sheet.
fn scale_length(width_m: f64) -> f64 {
    let target = width_m / 5.0;
    let decade = 10f64.powf(target.log10().floor());
    [5.0, 2.0, 1.0].into_iter().map(|k| k * decade).find(|l| *l <= target).unwrap_or(decade)
}

/// Draw the sheet and encode it as PNG.
///
/// # Errors
/// A sheet scale giving an empty or oversized image, or a PNG encoding error.
pub fn render(prepared: &Prepared, run: &CultureRun, m_per_px: f64) -> Result<Vec<u8>, String> {
    let grid = &prepared.grid;
    let frame = grid.frame;
    let side = (frame.width_m / m_per_px).round() as u32;
    let dims = IntSize::from_wh(side, side).ok_or(format!("sheet of {side} px"))?;
    // Terrain and water, nearest grid cell per pixel.
    let (lo, hi) = grid.heights.iter().zip(&grid.water).filter(|(_, w)| **w == 0).fold((f32::MAX, f32::MIN), |(lo, hi), (h, _)| (lo.min(*h), hi.max(*h)));
    let span = (hi - lo).max(f32::EPSILON);
    let mut data = Vec::with_capacity(dims.width() as usize * dims.height() as usize * 4);
    for py in 0..side {
        for px in 0..side {
            let col = ((f64::from(px) + 0.5) * m_per_px / frame.width_m * frame.n as f64) as usize;
            let row = ((f64::from(py) + 0.5) * m_per_px / frame.width_m * frame.n as f64) as usize;
            let i = row.min(frame.n - 1) * frame.n + col.min(frame.n - 1);
            let rgb = if grid.water[i] != 0 { WATER } else { mix(LAND_LOW, LAND_HIGH, (grid.heights[i] - lo) / span) };
            data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    let mut pix = Pixmap::from_vec(data, dims).ok_or("sheet buffer")?;
    let s = (1.0 / m_per_px) as f32;
    let id = Transform::identity();
    if let Some(kept) = &prepared.kept {
        for land in &kept.claim.land {
            if let Some(path) = shape_path(land, s) {
                pix.fill_path(&path, &paint(KEPT_LAND, 255), FillRule::EvenOdd, id, None);
            }
        }
        for footprint in &kept.footprints {
            if let Some(path) = shape_path(footprint, s) {
                pix.fill_path(&path, &paint(KEPT_FOOTPRINT, 255), FillRule::EvenOdd, id, None);
            }
        }
        for exit in &kept.claim.streets {
            let mut pb = PathBuilder::new();
            for (i, p) in exit.path.iter().enumerate() {
                let (x, y) = (p.x as f32 * s, p.y as f32 * s);
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            let width_m = exit.widths.first().copied().unwrap_or(0.0) as f32;
            if let Some(path) = pb.finish() {
                let stroke = Stroke { width: (width_m * s).max(MIN_STROKE_PX), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
                pix.stroke_path(&path, &paint(KEPT_EXIT, 255), &stroke, id, None);
            }
        }
    }
    for block in &run.layout.blocks {
        if let Some(path) = shape_path(&block.shape, s) {
            pix.fill_path(&path, &paint(BLOCK, 255), FillRule::EvenOdd, id, None);
        }
    }
    let edge = Stroke { width: 0.6, line_join: LineJoin::Round, ..Stroke::default() };
    for lot in &run.partition.parcels {
        if let Some(path) = shape_path(&lot.shape, s) {
            let fill = LOT_USES.iter().find(|(u, _)| *u == lot.use_kind).map_or(LOT_OTHER, |(_, c)| *c);
            pix.fill_path(&path, &paint(fill, 255), FillRule::EvenOdd, id, None);
            pix.stroke_path(&path, &paint(LOT_EDGE, 255), &edge, id, None);
        }
    }
    // Streets, minor ranks first so main streets draw on top, each at its mean width.
    let mut streets: Vec<_> = run.layout.streets.iter().filter(|st| st.path.len() >= 2).collect();
    streets.sort_by_key(|st| std::cmp::Reverse(st.rank));
    for st in streets {
        let mut pb = PathBuilder::new();
        for (i, p) in st.path.iter().enumerate() {
            let (x, y) = (p.x as f32 * s, p.y as f32 * s);
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        let Some(path) = pb.finish() else { continue };
        let width_m = if st.widths.is_empty() { 0.0 } else { st.widths.iter().sum::<f64>() / st.widths.len() as f64 };
        let stroke = Stroke { width: ((width_m as f32) * s).max(MIN_STROKE_PX), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        let colour = STREET_RANKS[st.rank.min(STREET_RANKS.len() - 1)];
        pix.stroke_path(&path, &paint(colour, 255), &stroke, id, None);
    }
    // Scale bar, bottom left: four alternating segments on a white plate, with its length in metres.
    let length_m = scale_length(frame.width_m);
    let bar_px = (length_m / m_per_px) as f32;
    let label = format!("{length_m:.0}m");
    let label_px = label.chars().count() as f32 * 4.0 * GLYPH_PX;
    let (x0, y0) = (MARGIN_PX, side as f32 - MARGIN_PX - 6.0 * GLYPH_PX - 8.0);
    if let Some(plate) = Rect::from_xywh(x0 - 4.0, y0 - 4.0, bar_px.max(label_px) + 8.0, 6.0 * GLYPH_PX + 16.0) {
        pix.fill_rect(plate, &paint(PAPER, 220), id, None);
    }
    for k in 0..4u8 {
        let colour = if k % 2 == 0 { INK } else { PAPER };
        if let Some(r) = Rect::from_xywh(x0 + f32::from(k) * bar_px / 4.0, y0, bar_px / 4.0, 6.0) {
            pix.fill_rect(r, &paint(colour, 255), id, None);
        }
    }
    if let Some(r) = Rect::from_xywh(x0, y0, bar_px, 6.0) {
        pix.stroke_path(&PathBuilder::from_rect(r), &paint(INK, 255), &Stroke { width: 1.0, ..Stroke::default() }, id, None);
    }
    text(&mut pix, (x0, y0 + 12.0), &label);
    // North mark, top right.
    let (nx, ny) = (side as f32 - MARGIN_PX - 3.0 * GLYPH_PX, MARGIN_PX);
    let mut pb = PathBuilder::new();
    pb.move_to(nx + 1.5 * GLYPH_PX, ny);
    pb.line_to(nx + 3.0 * GLYPH_PX, ny + 5.0 * GLYPH_PX);
    pb.line_to(nx, ny + 5.0 * GLYPH_PX);
    pb.close();
    if let Some(path) = pb.finish() {
        pix.fill_path(&path, &paint(INK, 255), FillRule::Winding, id, None);
    }
    text(&mut pix, (nx, ny + 6.0 * GLYPH_PX), "N");
    pix.encode_png().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn scale_bar_is_a_round_fifth_or_less() {
        assert!((super::scale_length(2000.0) - 200.0).abs() < 1e-9);
        assert!((super::scale_length(1000.0) - 200.0).abs() < 1e-9);
        assert!((super::scale_length(600.0) - 100.0).abs() < 1e-9);
        assert!((super::scale_length(3000.0) - 500.0).abs() < 1e-9);
    }
}
