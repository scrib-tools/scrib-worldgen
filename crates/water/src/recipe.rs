//! A water recipe (`recipes/water-*.toml`). Every value is a tunable carrying its unit and source (`Guess` allowed),
//! as in Scrib's `superfargoth/docs/specs/tunables.md`; none lives in code.
//!
//! Magna Urbis's own options are passed through by name (camelCase, as Magna Urbis's serde names them):
//! `[settings.<key>]` fills its `vector_water::Settings` (`vector_water/mod.rs:71-109`; no default in Magna Urbis, so
//! every required key must be given), `[conditioning.<key>]` its `Conditioning` (`conditioning.rs:5-40`; absent keys
//! take Magna Urbis's `Default`). A key Magna Urbis does not know is refused.
use magna_urbis_terrain::vector_water::{Climate, Conditioning, Settings};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One recipe value with its unit and where it comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tunable<T> {
    pub value: T,
    pub unit: String,
    pub source: String,
}

/// The climate Magna Urbis's water budget reads (`vector_water/mod.rs:131`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClimateRecipe {
    /// Sea-level mean temperature (°C); Magna Urbis lowers it with altitude.
    pub temperature: Tunable<f64>,
    /// Yearly precipitation (mm).
    pub precipitation: Tunable<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Seed string handed to Magna Urbis (meanders, hollows); same guide, same recipe = same output.
    pub seed: Tunable<String>,
    /// Integer factor the padded square is box-averaged by before the chain (1 = full guide resolution); the
    /// height change is brought back with Magna Urbis's own `upsample_delta`.
    pub downsample: Tunable<u32>,
    /// Depth (m below sea) of the cells that pad the guide to a square.
    pub pad_depth_m: Tunable<f64>,
    /// Douglas-Peucker tolerance (m) of the lake outlines (traced as the shoreline iso-contour of the carved heights).
    pub lake_simplify_m: Tunable<f64>,
    /// Distance (m) a lake outline is pushed out past its shoreline, under the bank.
    pub lake_overlap_m: Tunable<f64>,
    /// Share of a river's bed depth filled with water: the surface sits at the carved centre ground plus
    /// `river_fill · bed_depth(width)`.
    pub river_fill: Tunable<f64>,
    /// Largest dry closed basin (km²) a river course crossing it at its spill level fills as a lake; a larger one
    /// (or one touching a stamp or other water) ends the course at the basin's rim.
    pub max_basin_lake_km2: Tunable<f64>,
    pub climate: ClimateRecipe,
    /// Magna Urbis `Settings`, by its camelCase key.
    pub settings: BTreeMap<String, Tunable<toml::Value>>,
    /// Magna Urbis `Conditioning`, by its camelCase key.
    #[serde(default)]
    pub conditioning: BTreeMap<String, Tunable<toml::Value>>,
}

fn check<T>(name: &str, t: &Tunable<T>, ok: bool, want: &str) -> Result<(), String> {
    if t.unit.trim().is_empty() || t.source.trim().is_empty() {
        return Err(format!("recipe `{name}`: unit and source are required"));
    }
    if ok { Ok(()) } else { Err(format!("recipe `{name}`: {want}")) }
}

/// Build a Magna Urbis options struct from the recipe's pass-through table; refuses keys it does not serialize back.
fn pass_through<T: DeserializeOwned + Serialize>(
    table: &str,
    entries: &BTreeMap<String, Tunable<toml::Value>>,
) -> Result<T, String> {
    let mut map = serde_json::Map::new();
    for (key, t) in entries {
        check(&format!("{table}.{key}"), t, true, "")?;
        let v = serde_json::to_value(&t.value).map_err(|e| format!("recipe `{table}.{key}`: {e}"))?;
        map.insert(key.clone(), v);
    }
    let out: T = serde_json::from_value(serde_json::Value::Object(map)).map_err(|e| format!("recipe `[{table}]`: {e}"))?;
    let known = serde_json::to_value(&out).map_err(|e| e.to_string())?;
    for key in entries.keys() {
        if known.get(key).is_none() {
            return Err(format!("recipe `{table}.{key}`: not a Magna Urbis key"));
        }
    }
    Ok(out)
}

impl Recipe {
    /// Parse and validate a recipe.
    ///
    /// # Errors
    /// TOML errors, a missing unit or source, a value out of range, an unknown or missing Magna Urbis key.
    pub fn parse(text: &str) -> Result<Self, String> {
        let recipe: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        recipe.validate()?;
        Ok(recipe)
    }

    /// # Errors
    /// See [`Recipe::parse`].
    pub fn validate(&self) -> Result<(), String> {
        check("seed", &self.seed, !self.seed.value.is_empty(), "not empty")?;
        check("downsample", &self.downsample, self.downsample.value >= 1, "1 or more")?;
        let p = self.pad_depth_m.value;
        check("pad_depth_m", &self.pad_depth_m, p.is_finite() && p > 0.0, "more than 0")?;
        let l = self.lake_simplify_m.value;
        check("lake_simplify_m", &self.lake_simplify_m, l.is_finite() && l >= 0.0, "0 or more")?;
        let o = self.lake_overlap_m.value;
        check("lake_overlap_m", &self.lake_overlap_m, o.is_finite() && o >= 0.0, "0 or more")?;
        let f = self.river_fill.value;
        check("river_fill", &self.river_fill, (0.0..=1.0).contains(&f), "0 to 1")?;
        let b = self.max_basin_lake_km2.value;
        check("max_basin_lake_km2", &self.max_basin_lake_km2, b.is_finite() && b >= 0.0, "0 or more")?;
        let c = &self.climate;
        check("climate.temperature", &c.temperature, c.temperature.value.is_finite(), "finite")?;
        let pr = c.precipitation.value;
        check("climate.precipitation", &c.precipitation, pr.is_finite() && pr >= 0.0, "0 or more")?;
        // Magna Urbis's own wire check (`vector_water/wire.rs:480-510`): positive river area and width scale.
        let s = self.mu_settings()?;
        if !(s.river_area.is_finite() && s.river_area > 0.0) {
            return Err("recipe `settings.riverArea`: more than 0".into());
        }
        if !(s.width_scale.is_finite() && s.width_scale > 0.0) {
            return Err("recipe `settings.widthScale`: more than 0".into());
        }
        self.mu_conditioning()?;
        Ok(())
    }

    /// Magna Urbis `Settings` from `[settings]`.
    ///
    /// # Errors
    /// A missing required key, an unknown key, a wrong type.
    pub fn mu_settings(&self) -> Result<Settings, String> {
        pass_through("settings", &self.settings)
    }

    /// Magna Urbis `Conditioning` from `[conditioning]`.
    ///
    /// # Errors
    /// An unknown key, a wrong type.
    pub fn mu_conditioning(&self) -> Result<Conditioning, String> {
        pass_through("conditioning", &self.conditioning)
    }

    #[must_use]
    pub fn mu_climate(&self) -> Climate {
        Climate { temperature: self.climate.temperature.value, precipitation: self.climate.precipitation.value }
    }
}
