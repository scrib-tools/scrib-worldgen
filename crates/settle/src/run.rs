//! One culture's run: Magna Urbis's quarters (streets, blocks) then parcels (lots) on the cropped grid
//! (`urban/mod.rs:104` `generate_quarters`, `:124` `generate_parcels`), and the layout folder it writes.
use crate::claim::{self, Kept};
use crate::grid::{CropSpec, Frame, Grid};
use crate::recipe::Recipe;
use crate::site::SiteRaster;
use crate::{crossing, sheet, stats};
use magna_urbis_core::settlements::extent_radius;
use magna_urbis_core::urban::neighbours::Claim;
use magna_urbis_core::urban::{self, Layout, LayoutConfig, ParcelConfig, Partition, Point, Site, Street};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// `layout.json` format version.
pub const LAYOUT_FORMAT: u32 = 1;
/// The Magna Urbis commit this tool is built on (`Cargo.toml`).
pub const MAGNA_URBIS_REV: &str = "21e22171f0b1766650b2cfef5d4bdaf574a1d325";
/// Magna Urbis site key of the one town a run lays out (its per-site seed stream derives from it,
/// `pipeline/mod.rs:84`).
pub const SITE_KEY: &str = "settle";

/// What a site's surface and frame give every culture run.
pub struct Prepared {
    pub grid: Grid,
    /// Town centre in Magna Urbis's local frame.
    pub centre: Point,
    /// The site's kept town (arm A), passed to Magna Urbis as a neighbour's claim.
    pub kept: Option<Kept>,
    /// Regional roads the town is laid along (Magna Urbis's `roads`, `urban/mod.rs:107`): its founding crossing
    /// (`crate::crossing`), if the recipe asks for one.
    pub roads: Vec<Street>,
}

/// Crop the site to the recipe's planning grid.
///
/// # Errors
/// See [`Grid::crop`]; an unknown water class; a site with a claim and a recipe without a `[claim]` table.
pub fn prepare(site: &SiteRaster, recipe: &Recipe) -> Result<Prepared, String> {
    let code = recipe.water_code().ok_or("unknown water class")?;
    let spec = CropSpec {
        radius_m: recipe.crop_radius_m.value,
        spacing_m: recipe.raster_spacing_m.value,
        water_code: code,
        relief_scale: recipe.relief_scale.value,
        min_water_area_m2: recipe.min_water_area_m2.value,
    };
    let grid = Grid::crop(site, site.meta.centre_m, &spec)?;
    let [cx, cy] = site.meta.centre_m;
    let centre = grid.frame.to_local(cx, cy);
    let kept = match (&site.meta.claim, &recipe.claim) {
        (None, _) => None,
        (Some(_), None) => return Err("the site carries a claim: the recipe needs a [claim] table".into()),
        (Some(_), Some(spec)) => claim::derive(site, &grid.frame, spec)?,
    };
    let roads = crossing::roads(&grid, centre, recipe.crossing.as_ref());
    Ok(Prepared { grid, centre, kept, roads })
}

/// The body of `layout.json`.
#[derive(Serialize)]
pub struct LayoutFile<'a> {
    pub format: u32,
    pub tool: String,
    pub magna_urbis_rev: &'static str,
    pub seed: &'a str,
    pub culture: &'a str,
    pub population: u32,
    pub site_radius_m: f64,
    pub site: SiteRef<'a>,
    /// Magna Urbis's local frame (x east, y south) in world metres.
    pub frame: Frame,
    pub recipe: &'a Recipe,
    /// The kept town's claim as passed to Magna Urbis (local frame), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<&'a Claim>,
    /// Magna Urbis's `Layout` and `Partition` as it serializes them, in its local frame.
    pub layout: &'a Layout,
    pub partition: &'a Partition,
}

#[derive(Serialize)]
pub struct SiteRef<'a> {
    pub name: &'a str,
    pub producer: &'a str,
    pub source: &'a str,
    pub water_level_m: f64,
}

pub struct CultureRun {
    pub culture: String,
    pub site_radius_m: f64,
    pub layout: Layout,
    pub partition: Partition,
    pub quarters_s: f64,
    pub parcels_s: f64,
}

fn options<T: serde::de::DeserializeOwned>(table: &toml::Table, what: &str) -> Result<T, String> {
    let json = serde_json::to_value(table).map_err(|e| e.to_string())?;
    serde_json::from_value(json).map_err(|e| format!("recipe magna_urbis.{what}: {e}"))
}

/// Run Magna Urbis's quarters and parcels for one culture.
///
/// # Errors
/// An unknown culture, bad Magna Urbis options, or Magna Urbis's own errors.
pub fn run_culture(prepared: &Prepared, recipe: &Recipe, culture: &str) -> Result<CultureRun, String> {
    let population = recipe.population.value;
    if urban::planned_land(culture, population).is_none() {
        return Err(format!("culture `{culture}` is not in Magna Urbis's catalogue"));
    }
    let site_radius_m = if recipe.site_radius_m.value > 0.0 { recipe.site_radius_m.value } else { extent_radius(population) };
    let site = Site {
        key: SITE_KEY.into(),
        name: String::new(),
        population,
        culture: culture.into(),
        center: prepared.centre,
        radius: site_radius_m,
        // The kept town is a neighbour generated before this one: its land is never built on and its exits are
        // joined (`urban/neighbours.rs`). The site is not in `fixed_sites`: a fixed centre on the kept land could not
        // seed the growth (`footprint/growth.rs:322-337`), so Magna Urbis starts from the nearest free ground
        // (`neighbours::relocate`) and the new town grows around the kept one.
        neighbours: prepared.kept.iter().map(|k| k.claim.clone()).collect(),
    };
    let layout_config: LayoutConfig = options(&recipe.magna_urbis.layout, "layout")?;
    let parcel_config: ParcelConfig = options(&recipe.magna_urbis.parcels, "parcels")?;
    let seed = recipe.seed.value.as_str();
    let start = Instant::now();
    let mut layouts = urban::generate_quarters(seed, &[site], &layout_config, prepared.grid.surface(), &prepared.roads)
        .map_err(|e| format!("{culture}: quarters: {e}"))?;
    let quarters_s = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let mut partitions = urban::generate_parcels(seed, &layouts, &parcel_config).map_err(|e| format!("{culture}: parcels: {e}"))?;
    let parcels_s = start.elapsed().as_secs_f64();
    let (Some(layout), Some(partition)) = (layouts.pop(), partitions.pop()) else {
        return Err(format!("{culture}: Magna Urbis returned no layout"));
    };
    Ok(CultureRun { culture: culture.into(), site_radius_m, layout, partition, quarters_s, parcels_s })
}

/// `layout.json` text for a run (deterministic: no timings in it).
///
/// # Errors
/// Serialization errors.
pub fn layout_json(site: &SiteRaster, recipe: &Recipe, prepared: &Prepared, run: &CultureRun) -> Result<String, String> {
    let file = LayoutFile {
        format: LAYOUT_FORMAT,
        tool: format!("settle {}", env!("CARGO_PKG_VERSION")),
        magna_urbis_rev: MAGNA_URBIS_REV,
        seed: &recipe.seed.value,
        culture: &run.culture,
        population: recipe.population.value,
        site_radius_m: run.site_radius_m,
        site: SiteRef {
            name: &site.meta.name,
            producer: &site.meta.producer,
            source: &site.meta.source,
            water_level_m: site.meta.water_level_m,
        },
        frame: prepared.grid.frame,
        recipe,
        kept: prepared.kept.as_ref().map(|k| &k.claim),
        layout: &run.layout,
        partition: &run.partition,
    };
    serde_json::to_string(&file).map_err(|e| e.to_string())
}

/// Folder name of a culture's outputs.
#[must_use]
pub fn culture_dir(culture: &str) -> String {
    culture.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Read a site folder, run every culture of the recipe and write `<out>/<culture>/{layout.json, stats.json,
/// sheet.png}`. Returns each culture's stats.
///
/// # Errors
/// Any read, run or write error (the first one stops the call).
pub fn settle(site_dir: &Path, recipe: &Recipe, out: &Path) -> Result<Vec<Value>, String> {
    let site = SiteRaster::load(site_dir)?;
    let prepared = prepare(&site, recipe)?;
    let mut all = Vec::new();
    for culture in &recipe.cultures.value {
        let run = run_culture(&prepared, recipe, culture)?;
        let dir = out.join(culture_dir(culture));
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let write = |name: &str, bytes: &[u8]| fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"));
        write("layout.json", layout_json(&site, recipe, &prepared, &run)?.as_bytes())?;
        let stats = stats::stats(recipe, &prepared, &run);
        write("stats.json", serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?.as_bytes())?;
        let png = sheet::render(&prepared, &run, recipe.sheet_m_per_px.value)?;
        write("sheet.png", &png)?;
        all.push(stats);
    }
    Ok(all)
}
