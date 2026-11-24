//! Culture packs: TOML overlays of Magna Urbis's data documents (`recipes/cultures/*.toml`), merged once, before
//! Magna Urbis's first data read (`magna_urbis_core::data::install_packs`, vendor/magna-urbis/VENDOR.md patch 02).
//!
//! A pack file is the TOML form of a Magna Urbis pack: `{ "<document key>": <patch> }`, where a patch is a JSON
//! Merge Patch with the directives `"$from"` (deep copy of a node of the same document, then the rest merged onto
//! it) and `"$append"` (add missing items to an array). TOML converts to JSON one to one (tables are objects,
//! integers stay integers); `$` keys are quoted (`"$from" = "/presets/x~1core"`).
use crate::Recipe;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The recipe's pack files, resolved against the recipe's folder, in recipe order. None without `[culture_packs]`.
#[must_use]
pub fn paths(recipe: &Recipe, recipe_dir: &Path) -> Vec<PathBuf> {
    recipe.culture_packs.as_ref().map_or_else(Vec::new, |t| t.value.iter().map(|p| recipe_dir.join(p)).collect())
}

/// Reads one pack file and converts its TOML to JSON.
///
/// # Errors
/// The file cannot be read or is not TOML; the message names the file.
pub fn load(path: &Path) -> Result<Value, String> {
    let at = |e: String| format!("culture pack {}: {e}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| at(e.to_string()))?;
    toml::from_str::<Value>(&text).map_err(|e| at(e.to_string()))
}

/// Loads every pack file of `paths` and installs them into Magna Urbis, in order. Call once per process, before any
/// Magna Urbis call; nothing to install (no paths) changes nothing.
///
/// # Errors
/// A pack file that cannot be read or parsed, or that Magna Urbis refuses (unknown document, bad directive, `$from`
/// pointing at nothing, data already read); the message names the file(s).
pub fn install(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let packs = paths.iter().map(|p| load(p)).collect::<Result<Vec<_>, _>>()?;
    magna_urbis_core::data::install_packs(&packs).map_err(|e| {
        // Magna Urbis numbers packs from 0 in the order given.
        let names: Vec<String> = paths.iter().enumerate().map(|(i, p)| format!("pack {i} = {}", p.display())).collect();
        format!("culture packs ({}): {e}", names.join(", "))
    })
}
