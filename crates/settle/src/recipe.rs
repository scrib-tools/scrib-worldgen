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
    /// Culture packs (TOML, paths relative to the recipe file) merged into Magna Urbis's data before its first read,
    /// in order (`crate::culture_packs`); absent = Magna Urbis's built-in data.
    #[serde(default)]
    pub culture_packs: Option<Tunable<Vec<String>>>,
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
    /// Fitting kit buildings into the lots (`crate::fit`); absent = no fitting, no `placements.json`.
    #[serde(default)]
    pub fit: Option<FitRecipe>,
    /// Joining narrow empty lots along their street until a kit building fits (`crate::merge`, slice PL-3a); absent
    /// = no merging. Used only with fitting.
    #[serde(default)]
    pub merge: Option<MergeRecipe>,
    /// Closing the street wall between the buildings with kit walls, and lane mouths with arches (`crate::filler`,
    /// slice PL-3b); absent = no filler. Used only with fitting; needs `[merge]` (its frontage runs).
    #[serde(default)]
    pub filler: Option<FillerRecipe>,
    /// The town's streets as a road network v1 file (`streets.json`, `crate::streets`, slice S1a); absent = not
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streets: Option<StreetsRecipe>,
    /// Garden features on the lots no house went on and on Magna Urbis's garden lots (`crate::gardens`, a temporary
    /// scaffold, owner 2026-11-24); absent = none. Used only with fitting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gardens: Option<GardensRecipe>,
    #[serde(default)]
    pub magna_urbis: MagnaUrbisOptions,
}

/// Garden features (`crate::gardens`): temporary scaffold (owner 2026-11-24).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GardensRecipe {
    /// Write the garden features into `placements.json`.
    pub enabled: Tunable<bool>,
    /// An empty plot lot whose minimum-area box has a short side under this (m) is a `sliver`.
    pub sliver_width_m: Tunable<f64>,
}

impl GardensRecipe {
    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        check("gardens.enabled", &self.enabled, true, "")?;
        let w = self.sliver_width_m.value;
        check("gardens.sliver_width_m", &self.sliver_width_m, w.is_finite() && w >= 0.0, "0 or more")
    }

    /// The switch is on.
    #[must_use]
    pub fn on(&self) -> bool {
        self.enabled.value
    }
}

/// Catalogue best-fit of kit buildings into Magna Urbis's building footprints (`crate::fit`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitRecipe {
    /// `kit.json` to fit from (a path; `settle` resolves it against the recipe file's folder). Absent = only with
    /// `settle --kit`, else no fitting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<String>,
    /// Step (m) between the positions tried along the frontage.
    pub slide_step_m: Tunable<f64>,
    /// Most positions tried along the frontage per model and yaw (centre first, then outward).
    pub slide_max: Tunable<usize>,
    /// Distance (m) the door side stands back from the footprint's front edge.
    pub setback_m: Tunable<f64>,
    /// A hull may stand this far (m) outside its footprint.
    pub inside_tolerance_m: Tunable<f64>,
    /// Two hulls may interpenetrate this deep (m) and still count as touching.
    pub overlap_tolerance_m: Tunable<f64>,
    /// The pad height is the site ground this far (m) outside the street door.
    pub door_probe_m: Tunable<f64>,
    /// Spacing (m) of the site-ground samples under a hull for the pad step.
    pub ground_sample_m: Tunable<f64>,
    /// Ground rising more than this (m) above the pad under the hull buries the walls: rejected.
    pub pad_rise_max_m: Tunable<f64>,
    /// Candidates whose covered share is within this of the best one are picked by kit weight.
    pub score_margin: Tunable<f64>,
    /// A second door facing within this (degrees) of the first door's opposite is a back door: the model is also
    /// tried turned by 180 degrees.
    pub back_door_tolerance_deg: Tunable<f64>,
    /// What a building is fitted into (`crate::fit` module docs): `footprint` (Magna Urbis's building footprints),
    /// `lot` (the whole lot behind its street line, minus `lot_setback_m`) or `match` (one model per Magna Urbis
    /// building footprint, inside its lot, best intersection-over-union). Absent = `footprint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Tunable<FitDomain>>,
    /// With `domain = "lot"` (required then): distance (m) a hull keeps from every lot edge but the street side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_setback_m: Option<Tunable<f64>>,
    /// `match` domain fallback (`crate::fit` module docs): a building footprint no model matches at its front is
    /// tried again with the hull set back along the street normal in `slide_step_m` steps up to this far (m). Absent =
    /// no back-slide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back_slide_max_m: Option<Tunable<f64>>,
    /// `match` domain fallback, after the back-slide: the footprint's own oriented-box frame is tried (front, then
    /// set back) when its axis nearest the street normal is within this (degrees) of it. Absent = not tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_frame_max_deg: Option<Tunable<f64>>,
    /// `match` domain, last stage (`crate::fit` module docs): a building footprint still empty after the fallbacks
    /// is tried again at every stage with the ground under the hull allowed this far (m) above the pad (in place of
    /// `pad_rise_max_m`): the back wall sinks into the slope, the door stays at street level. Absent = not tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sink_back_max_m: Option<Tunable<f64>>,
    /// `match` domain, final stage `small` (`crate::fit` module docs): a building footprint still empty after every
    /// other stage is tried again at every stage with kit models of these kinds (not `building`) as the candidates,
    /// e.g. towers where no house fits. Absent = not tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_kinds: Option<Tunable<Vec<crate::formats::KitKind>>>,
}

/// What the fit pass fits a building into (`FitRecipe::domain`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitDomain {
    /// Magna Urbis's building footprints (`Lot::preview_parts`), each its own domain.
    #[default]
    Footprint,
    /// The whole lot, shrunk by `lot_setback_m` on every side but the street side.
    Lot,
    /// One kit model per Magna Urbis `building` footprint of a `plot` lot: hull inside the lot, best
    /// intersection-over-union with the footprint.
    Match,
}

impl FitRecipe {
    /// The lot setback (m) when the domain is `lot`, else `None` (footprint domain).
    #[must_use]
    pub fn lot_setback(&self) -> Option<f64> {
        match self.domain.as_ref().map(|d| d.value) {
            Some(FitDomain::Lot) => self.lot_setback_m.as_ref().map(|t| t.value),
            _ => None,
        }
    }

    /// The domain is `match`.
    #[must_use]
    pub fn match_domain(&self) -> bool {
        self.domain.as_ref().is_some_and(|d| d.value == FitDomain::Match)
    }

    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        let at_least_zero = |t: &Tunable<f64>| t.value.is_finite() && t.value >= 0.0;
        for (name, t) in [
            ("fit.slide_step_m", &self.slide_step_m),
            ("fit.door_probe_m", &self.door_probe_m),
            ("fit.ground_sample_m", &self.ground_sample_m),
        ] {
            check(name, t, positive(t), "more than 0")?;
        }
        for (name, t) in [
            ("fit.setback_m", &self.setback_m),
            ("fit.overlap_tolerance_m", &self.overlap_tolerance_m),
            ("fit.pad_rise_max_m", &self.pad_rise_max_m),
            ("fit.score_margin", &self.score_margin),
        ] {
            check(name, t, at_least_zero(t), "0 or more")?;
        }
        // Magna Urbis's parts stand up to 3 cm outside their lot: a tighter tolerance refuses its own drawing.
        let tol = self.inside_tolerance_m.value;
        check("fit.inside_tolerance_m", &self.inside_tolerance_m, tol.is_finite() && tol >= 0.05, "0.05 or more")?;
        check("fit.slide_max", &self.slide_max, self.slide_max.value > 0, "more than 0")?;
        let a = self.back_door_tolerance_deg.value;
        check(
            "fit.back_door_tolerance_deg",
            &self.back_door_tolerance_deg,
            a.is_finite() && (0.0..90.0).contains(&a),
            "in [0, 90)",
        )?;
        if let Some(d) = &self.domain {
            check("fit.domain", d, true, "")?;
        }
        if let Some(t) = &self.back_slide_max_m {
            check("fit.back_slide_max_m", t, at_least_zero(t), "0 or more")?;
        }
        if let Some(t) = &self.part_frame_max_deg {
            // The box axis nearest the street normal is never more than 45 degrees off it.
            check("fit.part_frame_max_deg", t, t.value.is_finite() && (0.0..=45.0).contains(&t.value), "in [0, 45]")?;
        }
        if let Some(t) = &self.sink_back_max_m {
            check("fit.sink_back_max_m", t, at_least_zero(t), "0 or more")?;
        }
        if let Some(t) = &self.small_kinds {
            let ok = !t.value.is_empty() && !t.value.contains(&crate::formats::KitKind::Building);
            check("fit.small_kinds", t, ok, "one kit kind or more, not `building`")?;
        }
        match (self.domain.as_ref().map(|d| d.value), &self.lot_setback_m) {
            (Some(FitDomain::Lot), None) => Err("recipe `fit.lot_setback_m`: required with `fit.domain = \"lot\"`".into()),
            (_, Some(t)) => check("fit.lot_setback_m", t, at_least_zero(t), "0 or more"),
            _ => Ok(()),
        }
    }
}

/// Joining narrow empty lots on one frontage under one kit building (`crate::merge`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeRecipe {
    /// Most lots joined under one building.
    pub max_lots: Tunable<usize>,
    /// Most frontage (m, the members' access segments summed) joined under one building.
    pub max_front_m: Tunable<f64>,
    /// Two lots are neighbours only when they share at least this much boundary (m).
    pub min_shared_m: Tunable<f64>,
    /// Two lot edges this close (m) count as one shared boundary.
    pub shared_tolerance_m: Tunable<f64>,
    /// Most angle (degrees) between two neighbours' street normals.
    pub max_angle_deg: Tunable<f64>,
    /// Most distance (m) of a neighbour's frontage midpoint from the other's frontage line.
    pub max_offset_m: Tunable<f64>,
    /// Most gap (m) between two neighbours' frontages along the street.
    pub max_gap_m: Tunable<f64>,
}

impl MergeRecipe {
    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        let at_least_zero = |t: &Tunable<f64>| t.value.is_finite() && t.value >= 0.0;
        check("merge.max_lots", &self.max_lots, self.max_lots.value >= 2, "2 or more")?;
        for (name, t) in [("merge.max_front_m", &self.max_front_m), ("merge.min_shared_m", &self.min_shared_m)] {
            check(name, t, positive(t), "more than 0")?;
        }
        for (name, t) in [
            ("merge.shared_tolerance_m", &self.shared_tolerance_m),
            ("merge.max_offset_m", &self.max_offset_m),
            ("merge.max_gap_m", &self.max_gap_m),
        ] {
            check(name, t, at_least_zero(t), "0 or more")?;
        }
        let a = self.max_angle_deg.value;
        check("merge.max_angle_deg", &self.max_angle_deg, a.is_finite() && (0.0..90.0).contains(&a), "in [0, 90)")
    }
}

/// The street wall's filler: walls along the lot faces no building covers, arches across lane mouths (`crate::filler`).
/// Lot faces and frontage runs follow `[merge]` (`max_offset_m` is the street-face reach).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillerRecipe {
    /// Kit model ids of the wall pieces, in order of preference (their local x along the wall, outer face on local
    /// +y).
    pub walls: Tunable<Vec<String>>,
    /// Kit model ids of the arches (same frame); may be empty.
    pub arches: Tunable<Vec<String>>,
    /// A building hull covers a lot face when it reaches within this depth (m) behind the face line.
    pub cover_depth_m: Tunable<f64>,
    /// Uncovered face stretches shorter than this (m) stay open.
    pub min_gap_m: Tunable<f64>,
    /// A wall run may overhang a building by this much (m) at either end, and two pieces may overlap this much; a
    /// piece may interpenetrate a placed hull this deep.
    pub overlap_max_m: Tunable<f64>,
    /// Wall combinations whose summed lengths differ by less than this (m) are equally good: the one using pieces
    /// earlier in `walls` wins.
    pub length_tie_m: Tunable<f64>,
    /// A lane mouth (gap between two frontage runs on one street line) gets an arch when its width lies within the
    /// arches' widths widened by this (m) each way.
    pub arch_slack_m: Tunable<f64>,
    /// Kit model ids of the towers (same frame), in order of preference; absent = no towers. Needs `tower_turn_deg`
    /// and `tower_min_spacing_m`.
    #[serde(default)]
    pub towers: Option<Tunable<Vec<String>>>,
    /// A run whose street face turns more than this (degrees) between two members gets a tower at that corner.
    #[serde(default)]
    pub tower_turn_deg: Option<Tunable<f64>>,
    /// Least distance (m) between two towers' corner points.
    #[serde(default)]
    pub tower_min_spacing_m: Option<Tunable<f64>>,
}

impl FillerRecipe {
    /// # Errors
    /// A missing unit or source, a value out of range.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        let at_least_zero = |t: &Tunable<f64>| t.value.is_finite() && t.value >= 0.0;
        check("filler.walls", &self.walls, !self.walls.value.is_empty(), "at least one model id")?;
        check("filler.arches", &self.arches, true, "")?;
        check("filler.min_gap_m", &self.min_gap_m, positive(&self.min_gap_m), "more than 0")?;
        for (name, t) in [
            ("filler.cover_depth_m", &self.cover_depth_m),
            ("filler.overlap_max_m", &self.overlap_max_m),
            ("filler.length_tie_m", &self.length_tie_m),
            ("filler.arch_slack_m", &self.arch_slack_m),
        ] {
            check(name, t, at_least_zero(t), "0 or more")?;
        }
        let Some(towers) = &self.towers else { return Ok(()) };
        check("filler.towers", towers, !towers.value.is_empty(), "at least one model id")?;
        let need = |name: &str| format!("recipe `filler.{name}` is required with `filler.towers`");
        let turn = self.tower_turn_deg.as_ref().ok_or_else(|| need("tower_turn_deg"))?;
        let a = turn.value;
        check("filler.tower_turn_deg", turn, a.is_finite() && (0.0..180.0).contains(&a), "in [0, 180)")?;
        let spacing = self.tower_min_spacing_m.as_ref().ok_or_else(|| need("tower_min_spacing_m"))?;
        check("filler.tower_min_spacing_m", spacing, at_least_zero(spacing), "0 or more")
    }
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
        if let Some(packs) = &self.culture_packs {
            let ok = !packs.value.is_empty() && packs.value.iter().all(|p| !p.trim().is_empty());
            check("culture_packs", packs, ok, "at least one pack path, none empty")?;
        }
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
        self.crossing.as_ref().map_or(Ok(()), CrossingRecipe::validate)?;
        self.fit.as_ref().map_or(Ok(()), FitRecipe::validate)?;
        self.merge.as_ref().map_or(Ok(()), MergeRecipe::validate)?;
        if self.filler.is_some() && self.merge.is_none() {
            return Err("recipe `[filler]` needs a `[merge]` table (its frontage runs)".into());
        }
        self.filler.as_ref().map_or(Ok(()), FillerRecipe::validate)?;
        self.gardens.as_ref().map_or(Ok(()), GardensRecipe::validate)?;
        self.streets.as_ref().map_or(Ok(()), StreetsRecipe::validate)
    }

    /// Magna Urbis's water code for `water_class`.
    #[must_use]
    pub fn water_code(&self) -> Option<u8> {
        WATER_CLASSES.iter().find(|(name, _)| *name == self.water_class.value).map(|(_, code)| *code)
    }
}

/// Road network v1 classes (Scrib `sf_worldgen::roads`).
pub const ROAD_CLASSES: [&str; 5] = ["highway", "main", "local", "track", "trail"];

/// A street's road network class and surface (`crate::streets`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreetStyle {
    /// `highway | main | local | track | trail`.
    pub class: String,
    /// Free string; the game reads `paved`, `flagstone` and `cobble` as paved, anything else as dirt.
    pub surface: String,
}

/// How Magna Urbis's planned streets become `streets.json` (`crate::streets`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreetsRecipe {
    /// Class and surface per Magna Urbis street role (`Street.role`); a role not listed takes `by_rank`.
    pub by_role: Tunable<std::collections::BTreeMap<String, StreetStyle>>,
    /// Class and surface per Magna Urbis street rank (`Street.rank`, index = rank; a rank past the end takes the last).
    pub by_rank: Tunable<Vec<StreetStyle>>,
    /// Street ends this close (m) are one junction.
    pub junction_merge_m: Tunable<f64>,
    /// Spacing (m) of the water samples along a street.
    pub water_sample_m: Tunable<f64>,
    /// A bridge over the site's water reaches this far (m) past the last wet sample on each side.
    pub bridge_pad_m: Tunable<f64>,
    /// Run (m) over which the steepest grade is measured along a street.
    pub grade_run_m: Tunable<f64>,
    /// A street whose every water sample lies this close (m) to a regional road the town was laid along is flagged
    /// `regional`.
    pub regional_match_m: Tunable<f64>,
}

impl StreetsRecipe {
    /// The style of a Magna Urbis street (role first, then rank).
    #[must_use]
    pub fn style(&self, role: &str, rank: usize) -> &StreetStyle {
        self.by_role.value.get(role).unwrap_or_else(|| {
            let ranks = &self.by_rank.value;
            &ranks[rank.min(ranks.len() - 1)]
        })
    }

    /// # Errors
    /// A missing unit or source, a value out of range, an unknown class, an empty surface.
    pub fn validate(&self) -> Result<(), String> {
        let ok = |s: &StreetStyle| ROAD_CLASSES.contains(&s.class.as_str()) && !s.surface.trim().is_empty();
        let want = "classes highway, main, local, track or trail; surfaces not empty";
        check("streets.by_role", &self.by_role, self.by_role.value.values().all(ok), want)?;
        check("streets.by_rank", &self.by_rank, !self.by_rank.value.is_empty() && self.by_rank.value.iter().all(ok), want)?;
        let positive = |t: &Tunable<f64>| t.value.is_finite() && t.value > 0.0;
        for (name, t) in [
            ("streets.junction_merge_m", &self.junction_merge_m),
            ("streets.water_sample_m", &self.water_sample_m),
            ("streets.grade_run_m", &self.grade_run_m),
            ("streets.regional_match_m", &self.regional_match_m),
        ] {
            check(name, t, positive(t), "more than 0")?;
        }
        let pad = self.bridge_pad_m.value;
        check("streets.bridge_pad_m", &self.bridge_pad_m, pad.is_finite() && pad >= 0.0, "0 or more")
    }
}
