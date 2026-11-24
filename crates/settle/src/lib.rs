//! `settle`: grow or found a settlement on a site folder with Magna Urbis (`docs/specs/settlements.md`, slice ST-3a).
//!
//! Site folder in (format 1), recipe in, layout folder out (`layout.json`, `stats.json`, `sheet.png` per culture).
//! Magna Urbis (github.com/dunkean/magna-urbis, AGPL-3.0-only) is used unmodified, pinned to one commit.
pub mod claim;
pub mod crossing;
pub mod grid;
pub mod recipe;
pub mod run;
pub mod sheet;
pub mod site;
pub mod stats;

pub use recipe::Recipe;
pub use run::{CultureRun, Prepared, prepare, run_culture, settle};
pub use site::{SiteMeta, SiteRaster};
