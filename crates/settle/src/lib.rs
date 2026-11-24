//! `settle`: grow or found a settlement on a site folder with Magna Urbis (`docs/specs/settlements.md`, slice ST-3a).
//!
//! Site folder in (format 1), recipe in, layout folder out (`layout.json`, `stats.json`, `sheet.png` per culture).
//! Magna Urbis (github.com/dunkean/magna-urbis, AGPL-3.0-only) is used unmodified, pinned to one commit.
pub mod claim;
pub mod crossing;
pub mod culture_packs;
pub mod filler;
pub mod fit;
pub mod formats;
pub mod gardens;
pub mod grid;
pub mod lots;
pub mod merge;
pub mod recipe;
pub mod regional;
pub mod run;
pub mod sheet;
pub mod site;
pub mod stats;
pub mod streets;

pub use recipe::Recipe;
pub use regional::RegionalRoads;
pub use run::{CultureRun, KitSource, Prepared, prepare, prepare_with_roads, run_culture, settle, settle_with, settle_with_kit};
pub use site::{SiteMeta, SiteRaster};
