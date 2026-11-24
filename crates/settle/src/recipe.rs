//! A recipe (`recipes/*.toml`): what to grow on a site. Every value is a tunable carrying its unit and source
//! (`Guess` allowed), as in Scrib's `superfargoth/docs/specs/tunables.md`; none lives in code.
use serde::{Deserialize, Serialize};

/// One recipe value with its unit and where it comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tunable<T> {
    pub value: T,
    pub unit: String,
    pub source: String,
}

/// Magna Urbis's own options, passed through to its `LayoutConfig` / `ParcelConfig` (camelCase keys, as Magna
/// Urbis names them; unknown keys are refused by Magna Urbis). Empty = its defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagnaUrbisOptions {
    #[serde(default)]
    pub layout: toml::Table,
    #[serde(default)]
    pub parcels: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Magna Urbis culture ids, one run (and one output subfolder) each.
    pub cultures: Tunable<Vec<String>>,
    /// Target population (inhabitants).
    pub population: Tunable<u32>,
    /// Seed string; same seed, same site, same recipe = same layout.
    pub seed: Tunable<String>,
    /// Half side (m) of the square crop of the site handed to Magna Urbis, centred on `centre_m` and shrunk to fit
    /// the site raster.
    pub crop_radius_m: Tunable<f64>,
    /// Spacing (m) of the planning grid Magna Urbis reads (heights and water resampled to it).
    pub raster_spacing_m: Tunable<f64>,
    /// Town radius (m) given to Magna Urbis's `Site`; 0 = Magna Urbis's own `extent_radius(population)`.
    pub site_radius_m: Tunable<f64>,
    /// Magna Urbis water class of the site's open water: `sea`, `lake` or `river`.
    pub water_class: Tunable<String>,
    /// Vertical scale of the site's relief about its water level before planning (1 = heights as given): a game
    /// terrain steeper than real ground Magna Urbis's street grades were set for is planned at real-world grades.
    pub relief_scale: Tunable<f64>,
    /// Water bodies smaller than this (m², 4-connected on the site raster) plan as dry ground.
    pub min_water_area_m2: Tunable<f64>,
    /// Sheet scale (m per pixel).
    pub sheet_m_per_px: Tunable<f64>,
    /// How a site's kept content (`claim.footprints`) becomes Magna Urbis's `Claim` (`claim.rs`). Required when the
    /// site carries a claim; ignored otherwise.
    #[serde(default)]
    pub claim: Option<ClaimRecipe>,
    /// The founding crossing a site without regional roads gets (`crate::crossing`); absent = none.
    #[serde(default)]
    pub crossing: Option<CrossingRecipe>,
    #[serde(default)]
    pub magna_urbis: MagnaUrbisOptions,
}

/// The kept land and its exits, from the site's footprints and road texels (`claim.rs`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRecipe {
    /// Cell side (m) of the raster the kept land is closed on.
    pub cell_m: Tunable<f64>,
    /// Footprints grow by this (m) before closing: the yard and eaves a kept building holds.
    pub margin_m: Tunable<f64>,
    /// Closing radius (m): kept pieces closer than twice this (after the margin) merge into one outline.
    pub close_m: Tunable<f64>,
    /// Outline simplification tolerance (m).
    pub simplify_m: Tunable<f64>,
    /// Holes in the kept land smaller than this (m²) are filled.
    pub min_hole_m2: Tunable<f64>,
    /// Kept pieces smaller than this (m²) are dropped (stray props).
    pub min_land_m2: Tunable<f64>,
    /// Length (m) of an exit inside the kept land, along its road texels.
    pub exit_in_m: Tunable<f64>,
    /// Length (m) an exit is traced outward along its road texels.
    pub exit_out_m: Tunable<f64>,
    /// An exit whose road leaves the kept land by less than this (m) is dropped (a road along the edge).
    pub exit_min_m: Tunable<f64>,
    /// Exits whose outer ends lie closer than this (m) are one exit (the longer is kept).
    pub exit_merge_m: Tunable<f64>,
    /// Street width (m) of an exit.
    pub exit_width_m: Tunable<f64>,
    /// A new street ending within this (m) of an exit counts as joining it (`stats.json`).
    pub join_tolerance_m: Tunable<f64>,
}

impl ClaimRecipe {
    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        let at_least_zero = |t: &Tunable<f64>| t.value.is_finite() && t.value >= 0.0;
        for (name, t, ok) in [
            ("claim.cell_m", &self.cell_m, positive(&self.cell_m)),
            ("claim.margin_m", &self.margin_m, at_least_zero(&self.margin_m)),
            ("claim.close_m", &self.close_m, at_least_zero(&self.close_m)),
            ("claim.simplify_m", &self.simplify_m, at_least_zero(&self.simplify_m)),
            ("claim.min_hole_m2", &self.min_hole_m2, at_least_zero(&self.min_hole_m2)),
            ("claim.min_land_m2", &self.min_land_m2, at_least_zero(&self.min_land_m2)),
            ("claim.exit_in_m", &self.exit_in_m, at_least_zero(&self.exit_in_m)),
            ("claim.exit_out_m", &self.exit_out_m, positive(&self.exit_out_m)),
            ("claim.exit_min_m", &self.exit_min_m, at_least_zero(&self.exit_min_m)),
            ("claim.exit_merge_m", &self.exit_merge_m, at_least_zero(&self.exit_merge_m)),
            ("claim.exit_width_m", &self.exit_width_m, positive(&self.exit_width_m)),
            ("claim.join_tolerance_m", &self.join_tolerance_m, positive(&self.join_tolerance_m)),
        ] {
            check(name, t, ok, "in range (lengths and areas 0 or more; cell, outward length, width, tolerance more than 0)")?;
        }
        Ok(())
    }
}

/// Magna Urbis's planning water codes (`settlements/placement.rs:1120-1124`, `harbour_furniture.rs:270`).
/// The regional road across the river a town is founded on (`crate::crossing`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossingRecipe {
    /// Nearest water the crossing may start from (m from the centre).
    pub search_m: Tunable<f64>,
    /// Widest water crossed (m along the line).
    pub max_span_m: Tunable<f64>,
    /// Line directions tried (degrees apart).
    pub angle_step_deg: Tunable<f64>,
    /// Road length each way from the crossing's middle (m).
    pub reach_m: Tunable<f64>,
    pub width_m: Tunable<f64>,
    /// Magna Urbis street rank (0 highway, 1 main, 2 local).
    pub rank: Tunable<usize>,
}

impl CrossingRecipe {
    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        for (name, t) in [
            ("crossing.search_m", &self.search_m),
            ("crossing.max_span_m", &self.max_span_m),
            ("crossing.reach_m", &self.reach_m),
            ("crossing.width_m", &self.width_m),
        ] {
            check(name, t, positive(t), "more than 0")?;
        }
        let a = self.angle_step_deg.value;
        check("crossing.angle_step_deg", &self.angle_step_deg, a.is_finite() && a > 0.0 && a <= 90.0, "in (0, 90]")?;
        check("crossing.rank", &self.rank, self.rank.value <= 2, "0, 1 or 2")
    }
}

pub const WATER_CLASSES: [(&str, u8); 3] = [("sea", 1), ("lake", 2), ("river", 3)];

fn check<T>(name: &str, t: &Tunable<T>, ok: bool, want: &str) -> Result<(), String> {
    if t.unit.trim().is_empty() || t.source.trim().is_empty() {
        return Err(format!("recipe `{name}`: unit and source are required"));
    }
    if ok { Ok(()) } else { Err(format!("recipe `{name}`: {want}")) }
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
        let positive = |v: f64| v.is_finite() && v > 0.0;
        check("cultures", &self.cultures, !self.cultures.value.is_empty(), "at least one culture")?;
        check("population", &self.population, self.population.value > 0, "more than 0")?;
        check("seed", &self.seed, !self.seed.value.is_empty(), "not empty")?;
        check("crop_radius_m", &self.crop_radius_m, positive(self.crop_radius_m.value), "more than 0")?;
        check("raster_spacing_m", &self.raster_spacing_m, positive(self.raster_spacing_m.value), "more than 0")?;
        let r = self.site_radius_m.value;
        check("site_radius_m", &self.site_radius_m, r.is_finite() && r >= 0.0, "0 (auto) or more")?;
        check("water_class", &self.water_class, self.water_code().is_some(), "sea, lake or river")?;
        check("relief_scale", &self.relief_scale, positive(self.relief_scale.value), "more than 0")?;
        let a = self.min_water_area_m2.value;
        check("min_water_area_m2", &self.min_water_area_m2, a.is_finite() && a >= 0.0, "0 or more")?;
        check("sheet_m_per_px", &self.sheet_m_per_px, positive(self.sheet_m_per_px.value), "more than 0")?;
        self.claim.as_ref().map_or(Ok(()), ClaimRecipe::validate)?;
        self.crossing.as_ref().map_or(Ok(()), CrossingRecipe::validate)
    }

    /// Magna Urbis's water code for `water_class`.
    #[must_use]
    pub fn water_code(&self) -> Option<u8> {
        WATER_CLASSES.iter().find(|(name, _)| *name == self.water_class.value).map(|(_, code)| *code)
    }
}
