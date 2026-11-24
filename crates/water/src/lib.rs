//! `worldgen-water`: Magna Urbis's water chain on a Scrib guide folder.
//!
//! Guide folder in (format 1), recipe in; out: a copy of the guide whose `heights.f32` carries the chain's whole
//! relief change (hollows, conditioning, river beds and valleys) and whose `meta.json` gains a `water_from`
//! provenance key, plus `water.json` (format 1, guide frame): lakes as polygons with a level, rivers as
//! `[x, y, width_m, level_m]` polylines, both fitted to the carved heights (lake outlines on the shoreline pushed
//! under the bank, river levels in the carved bed). Lakes touching a height stamp are dropped and rivers are cut
//! where they enter one or a dropped lake (the runtime restores stamps, `mix(h_gen, h_stamp, w)`, so their ground
//! is not water). A dry closed basin that a course crosses at its spill level (Magna Urbis kept no lake there) is
//! filled as a lake at that level and the course split at it; one touching a stamp, larger than
//! `max_basin_lake_km2` or reaching other water ends the course at its rim instead.
//! Magna Urbis (github.com/dunkean/magna-urbis, AGPL-3.0-only) is used unmodified, pinned to one commit.
// Grid and geometry code: short coordinate names (x, y, c, r, w, h) read best.
#![allow(clippy::many_single_char_names, clippy::similar_names)]
pub mod guide;
pub mod polygon;
pub mod recipe;
pub mod run;

pub use recipe::Recipe;
pub use run::{Report, Stats, run};
