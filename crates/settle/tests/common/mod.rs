//! PL-0 checks of `lots.json` shared by the site fixtures: one entry per parcel with its id, every footprint inside
//! its lot, the types round-trip.
use magna_urbis_core::urban::{Partition, Point, Shape};
use magna_urbis_geom as g;
use settle::lots::{LotsFile, Xy};

/// How far (m) a footprint vertex may stand outside its lot: Magna Urbis fits footprints with its own geometric
/// tolerances (3 cm seen on the gorge fixture, european-organic); far below a building-scale overlap.
pub const INSIDE_TOLERANCE_M: f64 = 0.05;

fn shape(ring: &[Xy]) -> Shape {
    Shape { outer: ring.iter().map(|&[x, y]| Point { x, y }).collect(), holes: Vec::new() }
}

fn distance_to_ring(ring: &[Xy], p: Point) -> f64 {
    let pts: Vec<Point> = ring.iter().map(|&[x, y]| Point { x, y }).collect();
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = (dx * dx + dy * dy).max(1e-18);
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            p.distance(Point { x: a.x + dx * t, y: a.y + dy * t })
        })
        .fold(f64::INFINITY, f64::min)
}

/// Every vertex and edge midpoint of `ring` is inside `lot` or within the tolerance of its edge.
fn ring_inside(ring: &[Xy], lot: &[Xy]) -> Result<(), String> {
    let lot_shape = shape(lot);
    for (k, &start) in ring.iter().enumerate() {
        let end = ring[(k + 1) % ring.len()];
        for [x, y] in [start, [f64::midpoint(start[0], end[0]), f64::midpoint(start[1], end[1])]] {
            let p = Point { x, y };
            if !g::contains(&lot_shape, p) {
                let gap = distance_to_ring(lot, p);
                if gap > INSIDE_TOLERANCE_M {
                    return Err(format!("point ({x:.3}, {y:.3}) is {gap:.3} m outside"));
                }
            }
        }
    }
    Ok(())
}

/// Assert the PL-0 contract of `text` (a `lots.json`) against the run's partition; returns lots with a footprint.
pub fn check_lots(text: &str, partition: &Partition, culture: &str) -> usize {
    let file: LotsFile = serde_json::from_str(text).unwrap();
    assert_eq!(file.format, settle::lots::LOTS_FORMAT);
    assert_eq!(file.culture, culture);
    assert!(file.error.is_none(), "{culture}: requests failed: {:?}", file.error);
    // Exactly one entry per parcel, same ids, same order.
    let ids: Vec<&str> = file.lots.iter().map(|l| l.id.as_str()).collect();
    let parcels: Vec<&str> = partition.parcels.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, parcels, "{culture}: lots.json ids differ from the partition's parcels");
    let mut outside = Vec::new();
    for lot in &file.lots {
        for part in &lot.preview_parts {
            if let Err(e) = ring_inside(&part.polygon, &lot.polygon) {
                outside.push(format!("{} ({}): {e}", lot.id, part.label));
            }
        }
    }
    assert!(outside.is_empty(), "{culture}: {} footprints outside their lot: {:?}", outside.len(), &outside[..outside.len().min(5)]);
    // Round trip of the types: parse, write, parse again gives the same value.
    let again: LotsFile = serde_json::from_str(&settle::lots::lots_json(&file).unwrap()).unwrap();
    assert_eq!(again, file, "{culture}: lots.json does not round-trip");
    file.lots.iter().filter(|l| !l.preview_parts.is_empty()).count()
}
