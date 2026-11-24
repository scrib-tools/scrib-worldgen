//! `<guide>/water.json` (format 1, guide frame metres), written by the water tool: lakes as outlines with holes,
//! rivers as centrelines carrying width and water level per point. Only the keys read here are listed; others are
//! ignored.
use serde::Deserialize;
use std::path::Path;

/// `water.json` format this tool reads.
pub const WATER_FORMAT: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
pub struct WaterFile {
    pub format: u32,
    #[serde(default)]
    pub lakes: Vec<Lake>,
    #[serde(default)]
    pub rivers: Vec<River>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Lake {
    #[serde(default)]
    pub id: serde_json::Value,
    /// Water surface (m).
    pub level_m: f64,
    pub outline: Vec<[f64; 2]>,
    #[serde(default)]
    pub holes: Vec<Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct River {
    #[serde(default)]
    pub id: serde_json::Value,
    /// Centreline `[x, y, width_m, level_m]`, guide frame.
    pub points: Vec<[f64; 4]>,
}

impl WaterFile {
    /// `None` when the file is absent.
    ///
    /// # Errors
    /// An unreadable or malformed file, another format.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: Self = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if file.format != WATER_FORMAT {
            return Err(format!("{}: format {} (this tool reads {WATER_FORMAT})", path.display(), file.format));
        }
        Ok(Some(file))
    }
}
