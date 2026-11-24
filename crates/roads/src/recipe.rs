//! A roads recipe (`recipes/roads-*.toml`). Every value is a tunable carrying its unit and source (`Guess` allowed),
//! as in Scrib's `superfargoth/docs/specs/tunables.md`; none lives in code. The rest of Magna Urbis's `RoadConfig`
//! keeps its defaults (`roads/mod.rs:62-81`), except what an island without towns' surroundings must switch off
//! (`crate::run::road_config`).
use magna_urbis_core::settlements::roads::{LEVELS, PROFILES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One recipe value with its unit and where it comes from (same shape as `crates/settle/src/recipe.rs`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tunable<T> {
    pub value: T,
    pub unit: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Seed string; same guide, sites and recipe = same roads.
    pub seed: Tunable<String>,
    /// Side (cells) of the square Magna Urbis raster the guide is resampled to (16..=1024, `settlements/mod.rs:416`).
    pub raster_n: Tunable<usize>,
    /// Height (m below sea level) of the sea padding the guide out to a square.
    pub pad_depth_m: Tunable<f64>,
    /// Magna Urbis `RoadConfig.grid`: largest routing grid side (cells); the raster is pooled by whole blocks down to
    /// it (0 = route on the raster itself, else 64..=1024).
    pub grid: Tunable<usize>,
    /// `RoadConfig.density`: 0 = spanning tree only ... 1 = many loops.
    pub density: Tunable<f64>,
    /// `RoadConfig.level`: development level, one of Magna Urbis's `LEVELS` ids.
    pub level: Tunable<String>,
    /// `RoadConfig.sharing`: 0 = every route on its own ... 1 = strong trunk sharing.
    pub sharing: Tunable<f64>,
    /// `RoadConfig.crossingCost`: river and lake crossing cost multiplier (0.05..=10).
    pub crossing_cost: Tunable<f64>,
    /// Site culture (`sites.json`) -> Magna Urbis road profile id (`roads/profile.rs:29` `PROFILES`).
    pub profiles: Tunable<BTreeMap<String, String>>,
    /// Largest distance (m) from a site's wet centre to the dry routing cell it is routed from instead (its landing,
    /// `crate::ends::landing`); farther, the site is not routed from land.
    pub landing_search_m: Tunable<f64>,
}

fn check<T>(name: &str, t: &Tunable<T>, ok: bool, want: &str) -> Result<(), String> {
    if t.unit.trim().is_empty() || t.source.trim().is_empty() {
        return Err(format!("recipe `{name}`: unit and source are required"));
    }
    if ok { Ok(()) } else { Err(format!("recipe `{name}`: {want}")) }
}

/// Magna Urbis's pooling block for a raster of `n` cells and a routing grid of `grid` (`settlements/mod.rs:509-512`).
#[must_use]
pub fn pool_block(n: usize, grid: usize) -> usize {
    match grid {
        0 => 1,
        grid => (n.div_ceil(grid)..=n).find(|&k| n.is_multiple_of(k)).unwrap_or(1),
    }
}

impl Recipe {
    /// Parse and validate a recipe.
    ///
    /// # Errors
    /// TOML errors, a missing unit or source, a value out of range.
    pub fn parse(text: &str) -> Result<Self, String> {
        let recipe: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        recipe.validate()?;
        Ok(recipe)
    }

    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let unit = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
        check("seed", &self.seed, !self.seed.value.is_empty(), "not empty")?;
        let n = self.raster_n.value;
        check("raster_n", &self.raster_n, (16..=1024).contains(&n), "16 to 1024")?;
        let p = self.pad_depth_m.value;
        check("pad_depth_m", &self.pad_depth_m, p.is_finite() && p > 0.0, "more than 0")?;
        let g = self.grid.value;
        check("grid", &self.grid, g == 0 || (64..=1024).contains(&g), "0 or 64 to 1024")?;
        // A raster side with no small divisor would pool to a handful of cells (`pool_block`).
        let routed = n / pool_block(n, g);
        let pools_well = g == 0 || n <= g || routed >= g / 2;
        check("raster_n", &self.raster_n, pools_well, &format!("pools to {routed} cells for grid {g}: use a multiple of the grid"))?;
        check("density", &self.density, unit(self.density.value), "0 to 1")?;
        check("sharing", &self.sharing, unit(self.sharing.value), "0 to 1")?;
        let c = self.crossing_cost.value;
        check("crossing_cost", &self.crossing_cost, c.is_finite() && (0.05..=10.0).contains(&c), "0.05 to 10")?;
        let level = LEVELS.iter().any(|l| l.id == self.level.value);
        check("level", &self.level, level, "a Magna Urbis level id (primitive, rural, developed, imperial)")?;
        let search = self.landing_search_m.value;
        check("landing_search_m", &self.landing_search_m, search.is_finite() && search >= 0.0, "0 or more")?;
        check("profiles", &self.profiles, !self.profiles.value.is_empty(), "at least one culture")?;
        if let Some((culture, id)) = self.profiles.value.iter().find(|(_, id)| !PROFILES.iter().any(|p| p.id == id.as_str())) {
            let ids: Vec<&str> = PROFILES.iter().map(|p| p.id).collect();
            return Err(format!("recipe `profiles`: `{culture}` -> `{id}` is not a road profile ({})", ids.join(", ")));
        }
        Ok(())
    }
}
