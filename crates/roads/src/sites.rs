//! `sites.json` (format 1, guide frame metres): the settlements to link, fixed (Magna Urbis never moves a road site,
//! `settlements/mod.rs:3`).
use serde::{Deserialize, Serialize};
use std::path::Path;

/// `sites.json` format this tool reads.
pub const SITES_FORMAT: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SitesFile {
    pub format: u32,
    pub sites: Vec<SiteIn>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteIn {
    /// Unique key; junctions name their site by it.
    pub key: String,
    #[serde(default)]
    pub name: String,
    /// Inhabitants: sets the road class of the trips it makes and its Delaunay importance (`roads/mod.rs:202`).
    pub population: u32,
    /// Culture, mapped to a road profile by the recipe's `profiles`.
    pub culture: String,
    pub centre_m: [f64; 2],
    /// Settlement extent (m); 0 = Magna Urbis's own from the population (`settlements/mod.rs:655`).
    #[serde(default)]
    pub radius_m: f64,
}

impl SitesFile {
    /// # Errors
    /// An unreadable or malformed file, another format, a duplicate key.
    pub fn load(path: &Path) -> Result<Self, String> {
        let at = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| at(e.to_string()))?;
        let file: Self = serde_json::from_str(&text).map_err(|e| at(e.to_string()))?;
        if file.format != SITES_FORMAT {
            return Err(at(format!("format {} (this tool reads {SITES_FORMAT})", file.format)));
        }
        let mut keys: Vec<&str> = file.sites.iter().map(|s| s.key.as_str()).collect();
        keys.sort_unstable();
        if let Some(&[key, _]) = keys.windows(2).find(|w| w[0] == w[1]) {
            return Err(at(format!("site key `{key}` twice")));
        }
        Ok(file)
    }
}
