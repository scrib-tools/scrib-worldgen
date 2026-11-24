//! One culture's run: Magna Urbis's quarters (streets, blocks) then parcels (lots) on the cropped grid
//! (`urban/mod.rs:104` `generate_quarters`, `:124` `generate_parcels`), and the layout folder it writes.
use crate::claim::{self, Kept};
use crate::grid::{CropSpec, Frame, Grid};
use crate::recipe::Recipe;
use crate::site::SiteRaster;
use crate::formats::KitFile;
use crate::regional::{self, RegionalRoads};
use crate::{crossing, filler, fit, gardens, lots, sheet, stats, streets};
use magna_urbis_core::settlements::extent_radius;
use magna_urbis_core::urban::neighbours::Claim;
use magna_urbis_core::urban::{self, Layout, LayoutConfig, ParcelConfig, Partition, Point, Site, Street};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
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
    /// Regional roads the town is laid along (Magna Urbis's `roads`, `urban/mod.rs:107`): the guide's road network
    /// when a `roads.json` is given (`crate::regional`), else its founding crossing (`crate::crossing`), if the recipe
    /// asks for one.
    pub roads: Vec<Street>,
    /// Where `roads` came from, when a `roads.json` was given.
    pub regional: Option<RegionalRef>,
}

/// The `roads.json` a run's regional roads came from (`layout.json` `regional_roads`).
#[derive(Clone, Debug, Serialize)]
pub struct RegionalRef {
    /// BLAKE3 (hex) of the `roads.json` bytes.
    pub blake3: String,
    /// Regional streets kept on the crop, by role.
    pub streets_by_role: BTreeMap<String, usize>,
    /// Those streets (local frame), before Magna Urbis fits them (`crate::streets` flags the streets along them).
    #[serde(skip)]
    pub streets: Vec<Street>,
}

/// Crop the site to the recipe's planning grid.
///
/// # Errors
/// See [`Grid::crop`]; an unknown water class; a site with a claim and a recipe without a `[claim]` table.
pub fn prepare(site: &SiteRaster, recipe: &Recipe) -> Result<Prepared, String> {
    prepare_with_roads(site, recipe, None)
}

/// [`prepare`] with the guide's regional roads: given, the town is laid along them, plus the founding crossing when
/// the recipe has a `[crossing]` table.
///
/// # Errors
/// See [`prepare`].
pub fn prepare_with_roads(site: &SiteRaster, recipe: &Recipe, regional_roads: Option<&RegionalRoads>) -> Result<Prepared, String> {
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
    let (roads, regional) = match regional_roads {
        None => (crossing::roads(&grid, centre, recipe.crossing.as_ref()), None),
        Some(r) => {
            let mut roads = regional::roads(&grid, r);
            let streets_by_role = regional::by_role(&roads);
            let regional_streets = roads.clone();
            // The founding crossing stays when the recipe asks for one: the regional router never crosses water it
            // takes for sea (an inland channel at sea level, Balmora's Odai), so without it the town keeps one bank.
            roads.extend(crossing::roads(&grid, centre, recipe.crossing.as_ref()));
            (roads, Some(RegionalRef { blake3: r.blake3.clone(), streets_by_role, streets: regional_streets }))
        }
    };
    Ok(Prepared { grid, centre, kept, roads, regional })
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
    /// The guide's `roads.json` the regional roads came from, if one was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regional_roads: Option<&'a RegionalRef>,
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
        regional_roads: prepared.regional.as_ref(),
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

/// The kit a run fits buildings from (`settle --kit` or the recipe's `fit.kit`).
pub struct KitSource {
    pub kit: KitFile,
    /// BLAKE3 (hex) of the `kit.json` bytes.
    pub blake3: String,
    /// Recipe name written into `placements.json` (the recipe file's stem).
    pub recipe_name: String,
}

impl KitSource {
    /// Read a `kit.json`.
    ///
    /// # Errors
    /// I/O errors and everything [`KitFile::from_json`] refuses.
    pub fn load(path: &Path, recipe_name: &str) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let kit = KitFile::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self { kit, blake3: blake3::hash(text.as_bytes()).to_hex().to_string(), recipe_name: recipe_name.into() })
    }
}

/// Read a site folder, run every culture of the recipe and write `<out>/<culture>/{layout.json, lots.json,
/// stats.json, sheet.png}`. Returns each culture's stats. Fits buildings too when the recipe names a kit
/// (`fit.kit`, relative to the working directory here; `settle` resolves it against the recipe file).
///
/// # Errors
/// Any read, run or write error (the first one stops the call).
pub fn settle(site_dir: &Path, recipe: &Recipe, out: &Path) -> Result<Vec<Value>, String> {
    let kit = match recipe.fit.as_ref().and_then(|f| f.kit.as_deref()) {
        Some(path) => Some(KitSource::load(Path::new(path), "")?),
        None => None,
    };
    settle_with_kit(site_dir, recipe, out, kit.as_ref())
}

/// [`settle`] with the kit given: with one, also `placements.json` per culture (`crate::fit`).
///
/// # Errors
/// Any read, run or write error (the first one stops the call); a kit without a `[fit]` table in the recipe.
pub fn settle_with_kit(site_dir: &Path, recipe: &Recipe, out: &Path, kit: Option<&KitSource>) -> Result<Vec<Value>, String> {
    settle_with(site_dir, recipe, out, kit, None)
}

/// [`settle_with_kit`] with the guide's regional roads (`settle --roads`): given, the town is laid along them instead
/// of the founding crossing (`crate::regional`).
///
/// # Errors
/// See [`settle_with_kit`].
pub fn settle_with(
    site_dir: &Path,
    recipe: &Recipe,
    out: &Path,
    kit: Option<&KitSource>,
    regional_roads: Option<&RegionalRoads>,
) -> Result<Vec<Value>, String> {
    let fitting = match (kit, &recipe.fit) {
        (None, _) => None,
        (Some(_), None) => return Err("a kit was given: the recipe needs a [fit] table".into()),
        (Some(k), Some(f)) => Some((k, f)),
    };
    let site = SiteRaster::load(site_dir)?;
    let prepared = prepare_with_roads(&site, recipe, regional_roads)?;
    let mut all = Vec::new();
    for culture in &recipe.cultures.value {
        let run = run_culture(&prepared, recipe, culture)?;
        let dir = out.join(culture_dir(culture));
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let write = |name: &str, bytes: &[u8]| fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"));
        write("layout.json", layout_json(&site, recipe, &prepared, &run)?.as_bytes())?;
        let lots = lots::lots(&site, &recipe.seed.value, &prepared, &run);
        write("lots.json", lots::lots_json(&lots.file)?.as_bytes())?;
        let mut stats = stats::stats(recipe, &prepared, &run, &lots);
        if let Some(r) = &prepared.regional {
            stats["regional_roads_by_role"] = serde_json::to_value(&r.streets_by_role).map_err(|e| e.to_string())?;
        }
        if let Some((k, f)) = fitting {
            let ground = |x: f64, y: f64| f64::from(site.height_at(x, y));
            let (fitted, merged, filled) = filler::fit_merge_fill(
                &lots.file.lots,
                &k.kit,
                f,
                recipe.merge.as_ref(),
                recipe.filler.as_ref(),
                &recipe.seed.value,
                &ground,
            )?;
            let header = fit::Header {
                name: culture.clone(),
                producer: format!("settle {}", env!("CARGO_PKG_VERSION")),
                recipe: k.recipe_name.clone(),
                seed: recipe.seed.value.clone(),
                kit_blake3: k.blake3.clone(),
                origin_m: site.meta.centre_m,
            };
            let mut placements = fitted.file(header);
            if let Some(g) = recipe.gardens.as_ref().filter(|g| g.on()) {
                let planted = gardens::gardens(&lots.file.lots, &fitted.instances, &fitted.lots, g);
                stats["gardens"] = gardens::stats(&planted);
                placements.features.extend(planted.features);
            }
            placements.write(&dir)?;
            stats["fit"] = stats::fit(&fitted.stats);
            if let Some(m) = &merged {
                stats["merge"] = stats::merge(m);
            }
            if let Some(w) = &filled {
                stats["filler"] = stats::filler(w);
            }
        }
        if let Some(spec) = &recipe.streets {
            let regional = prepared.regional.as_ref().map_or(&[][..], |r| &r.streets[..]);
            let file = streets::streets(culture, &run.layout.streets, &prepared.grid.frame, &site, regional, spec);
            write(streets::STREETS_FILE, serde_json::to_string(&file).map_err(|e| e.to_string())?.as_bytes())?;
            stats["streets_export"] = streets::summary(&file);
        }
        write("stats.json", serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?.as_bytes())?;
        let png = sheet::render(&prepared, &run, recipe.sheet_m_per_px.value)?;
        write("sheet.png", &png)?;
        all.push(stats);
    }
    Ok(all)
}
