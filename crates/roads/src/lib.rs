//! `worldgen-roads`: the regional road network between given settlements on a Scrib guide, routed by Magna Urbis's
//! region stage (`settlements/mod.rs:506` `Terrain::roads`; research note `docs/research/magna-urbis-roads-2026-11-24.md`).
//!
//! Guide folder (format 1) + optional `water.json` + `sites.json` + recipe in, `roads.json` (format 1, guide frame
//! metres) out; a wet site centre is routed from its landing on land and every road stops on its sites' edges
//! (`ends`). Magna Urbis (github.com/dunkean/magna-urbis, AGPL-3.0-only) is used unmodified, pinned to one commit.
pub mod ends;
pub mod guide;
pub mod raster;
pub mod recipe;
pub mod run;
pub mod sites;
pub mod water;

pub use guide::Guide;
pub use raster::{Frame, MuRaster};
pub use recipe::Recipe;
pub use run::{RoadsFile, Summary, route};
pub use sites::SitesFile;
pub use water::WaterFile;
