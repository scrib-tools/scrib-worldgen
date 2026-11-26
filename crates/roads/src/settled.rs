//! A laid-out site's settled streets (`settle`'s `streets.json`, format 1, guide frame metres): inside that town the
//! streets are the road, so its regional roads are cut where they meet them (`crate::ends::clip_at_streets`).
use serde::Deserialize;
use std::path::Path;

/// `streets.json` format this tool reads.
pub const STREETS_FORMAT: u32 = 1;

/// One laid-out site's streets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settled {
    /// The site's key in `sites.json`.
    pub key: String,
    /// Street centrelines (guide frame metres).
    pub streets: Vec<Vec<[f64; 2]>>,
}

/// The part of `streets.json` read here; every other field is ignored.
#[derive(Deserialize)]
struct StreetsFile {
    format: u32,
    segments: Vec<StreetIn>,
}

#[derive(Deserialize)]
struct StreetIn {
    points: Vec<[f64; 2]>,
}

impl Settled {
    /// The streets of site `key` from `path`.
    ///
    /// # Errors
    /// An unreadable or malformed file, another format.
    pub fn load(key: &str, path: &Path) -> Result<Self, String> {
        let at = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| at(e.to_string()))?;
        let file: StreetsFile = serde_json::from_str(&text).map_err(|e| at(e.to_string()))?;
        if file.format != STREETS_FORMAT {
            return Err(at(format!("format {} (this tool reads {STREETS_FORMAT})", file.format)));
        }
        Ok(Self { key: key.to_string(), streets: file.segments.into_iter().map(|s| s.points).collect() })
    }
}
