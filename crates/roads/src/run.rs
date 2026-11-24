//! One run: guide + water + sites + recipe -> landings for wet site centres -> Magna Urbis `Terrain::roads`
//! (`settlements/mod.rs:506`) -> roads cut back to the sites' edges (`crate::ends`) -> `roads.json`.
use crate::ends::{self, Extent, Landing};
use crate::guide::Guide;
use crate::raster::{Frame, MuRaster};
use crate::recipe::{Recipe, pool_block};
use crate::sites::SitesFile;
use crate::water::WaterFile;
use magna_urbis_core::settlements::roads::{ExitCounts, RoadConfig, RoadOutput, RoadSite};
use magna_urbis_core::settlements::{Point, Terrain, extent_radius};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

/// `roads.json` format version.
pub const ROADS_FORMAT: u32 = 1;
/// The Magna Urbis commit this tool is built on (`Cargo.toml`).
pub const MAGNA_URBIS_REV: &str = "21e22171f0b1766650b2cfef5d4bdaf574a1d325";

/// One Magna Urbis settlement culture per road profile, which `roads::profile::profile_for` resolves to that profile
/// (`roads/profile.rs:158-181`): with `RoadConfig.culture = "auto"` the profile of a link is read from its larger
/// place's culture, and with network nodes, farms and tunnels off nothing else reads a road site's culture.
pub const PROFILE_CULTURE: [(&str, &str); 9] = [
    ("medieval", "european-organic"),
    ("roman", "roman"),
    ("east-asian", "chinese"),
    ("caravan", "persian"),
    ("andean", "inca"),
    ("mesoamerican", "aztec"),
    ("tribal", "nomad"),
    ("elven", "elven"),
    ("dwarven", "dwarven"),
];

/// The body of `roads.json` (guide frame metres; deterministic: no timings in it).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadsFile {
    pub format: u32,
    pub tool: String,
    pub magna_urbis_rev: String,
    pub seed: String,
    /// The Magna Urbis raster: `guide = (west_m + x, north_m - y)` of its local frame.
    pub frame: Frame,
    /// Side (m) of the cells Magna Urbis routed on (the raster pooled to `grid`).
    pub routing_cell_m: f64,
    /// The sites as routed, in `sites.json` order.
    pub sites: Vec<SiteOut>,
    pub segments: Vec<SegmentOut>,
    pub junctions: Vec<JunctionOut>,
    pub warnings: Vec<String>,
    /// Keys of the sites no segment reaches (no junction of theirs ends a segment).
    pub unreachable: Vec<String>,
    pub length_by_class_m: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteOut {
    pub key: String,
    pub centre_m: [f64; 2],
    /// Effective extent (m): `sites.json`'s, else Magna Urbis's from the population (`settlements/mod.rs:655`). Roads
    /// stop where they cross it (around the centre, and around the landing if any).
    pub radius_m: f64,
    /// A centre on water is routed from here instead: the nearest dry routing cell within `landing_search_m`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SegmentOut {
    pub id: usize,
    /// highway | main | local | track | trail
    pub class: String,
    pub surface: String,
    /// Magna Urbis road profile id.
    pub profile: String,
    pub width_m: f64,
    /// Junction ids at both ends.
    pub a: usize,
    pub b: usize,
    pub points: Vec<[f64; 2]>,
    pub crossings: Vec<CrossingOut>,
    pub length_m: f64,
    /// Length (m) cut off beyond each end (`a`, `b`) where the routed path ran on into a site; 0 = an end as routed.
    pub trimmed_m: [f64; 2],
    /// Steepest grade along the routed path, before the cut (rise over run).
    pub grade: f64,
    /// Population of the smaller end of the most important trip using this segment.
    pub importance: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossingOut {
    /// bridge | ford | causeway
    pub kind: String,
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JunctionOut {
    pub id: usize,
    pub pos: [f64; 2],
    pub degree: usize,
    /// `site` (a `sites.json` place), `access` (the end of one of Magna Urbis's field tracks into a place's farmed
    /// belt, `roads/mod.rs:540-600`), `exit` (map border; none with exits off) or `junction`.
    pub kind: String,
    /// Site key(s) at this junction (Magna Urbis names at most one).
    pub sites: Vec<String>,
}

/// What the summary line prints.
#[derive(Clone, Debug)]
pub struct Summary {
    pub n: usize,
    pub cell_m: f64,
    pub routing_cell_m: f64,
    pub seconds: f64,
    pub segments: usize,
    pub km_by_class: BTreeMap<String, f64>,
    pub crossings_by_kind: BTreeMap<String, usize>,
    pub unreachable: Vec<String>,
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let km: Vec<String> = self.km_by_class.iter().map(|(c, k)| format!("{c} {k:.1}")).collect();
        let cross: Vec<String> = self.crossings_by_kind.iter().map(|(k, c)| format!("{c} {k}")).collect();
        write!(
            f,
            "n {} at {:.1} m (routing {:.1} m), {:.1} s, {} segments, km: {}, crossings: {}, unreachable {}{}",
            self.n,
            self.cell_m,
            self.routing_cell_m,
            self.seconds,
            self.segments,
            if km.is_empty() { "none".into() } else { km.join(", ") },
            if cross.is_empty() { "none".into() } else { cross.join(", ") },
            self.unreachable.len(),
            if self.unreachable.is_empty() { String::new() } else { format!(" ({})", self.unreachable.join(", ")) },
        )
    }
}

/// Magna Urbis's `RoadConfig` from the recipe. Off: map exits (an island has no map-border gateways), activity access
/// (no farms or activity sites yet), network nodes (inns, tolls: not ours yet), tunnels (no Underdark).
#[must_use]
pub fn road_config(recipe: &Recipe) -> RoadConfig {
    RoadConfig {
        level: recipe.level.value.clone(),
        density: recipe.density.value,
        sharing: recipe.sharing.value,
        crossing_cost: recipe.crossing_cost.value,
        grid: recipe.grid.value,
        exits: ExitCounts { highway: 0, main: 0, local: 0, track: 0 },
        activity_access: false,
        network_nodes: false,
        tunnels: 0.0,
        ..RoadConfig::default()
    }
}

/// Road sites in Magna Urbis's local frame, each routed from its landing if it has one.
///
/// # Errors
/// A culture missing from the recipe's `profiles`, a centre outside the raster.
pub fn road_sites(sites: &SitesFile, recipe: &Recipe, frame: &Frame, landings: &[Landing]) -> Result<Vec<RoadSite>, String> {
    sites
        .sites
        .iter()
        .zip(landings)
        .map(|(s, landing)| {
            let profile = recipe.profiles.value.get(&s.culture).ok_or_else(|| {
                format!("site `{}`: culture `{}` is not in the recipe's `profiles`", s.key, s.culture)
            })?;
            let culture = PROFILE_CULTURE.iter().find(|(p, _)| p == profile).map(|(_, c)| *c).ok_or("unknown profile")?;
            let routed = if let Landing::Moved(p) = *landing { p } else { s.centre_m };
            let center = frame.to_local(routed);
            if !(center.x >= 0.0 && center.y >= 0.0 && center.x < frame.width_m && center.y < frame.width_m) {
                return Err(format!("site `{}`: centre {:?} is outside the guide", s.key, s.centre_m));
            }
            Ok(RoadSite {
                key: s.key.clone(),
                population: s.population,
                culture: culture.into(),
                center,
                radius: s.radius_m,
                ..RoadSite::default()
            })
        })
        .collect()
}

/// Everything a run produces.
pub struct Run {
    pub raster: MuRaster,
    pub file: RoadsFile,
    pub summary: Summary,
}

/// Route the roads between `sites` on the guide folder `guide_dir` (and its `water.json`, if any).
///
/// # Errors
/// Unreadable inputs, a site outside the guide or with an unmapped culture, Magna Urbis's own errors.
pub fn route(guide_dir: &Path, sites: &SitesFile, recipe: &Recipe) -> Result<Run, String> {
    let guide = Guide::load(guide_dir)?;
    let water = WaterFile::load(&guide_dir.join("water.json"))?;
    let raster = MuRaster::build(&guide, water.as_ref(), recipe.raster_n.value, recipe.pad_depth_m.value);
    let frame = raster.frame;
    let cfg = road_config(recipe);
    let k = pool_block(frame.n, cfg.grid);
    let landings: Vec<Landing> =
        sites.sites.iter().map(|s| ends::landing(&raster, k, s.centre_m, recipe.landing_search_m.value)).collect();
    let road_sites = road_sites(sites, recipe, &frame, &landings)?;
    let start = Instant::now();
    let terrain = Terrain::new(raster.raster())?;
    let out = terrain.roads(&recipe.seed.value, &road_sites, &cfg)?;
    let seconds = start.elapsed().as_secs_f64();
    #[allow(clippy::cast_precision_loss)] // a pooling block of at most 1024 cells
    let routing_cell_m = frame.cell_m * k as f64;
    let site_out: Vec<SiteOut> = sites
        .sites
        .iter()
        .zip(&landings)
        .map(|(s, l)| SiteOut {
            key: s.key.clone(),
            centre_m: s.centre_m,
            radius_m: if s.radius_m > 0.0 { s.radius_m } else { extent_radius(s.population) },
            landing: if let Landing::Moved(p) = *l { Some(p) } else { None },
        })
        .collect();
    let mut file = convert(&out, &frame, recipe, routing_cell_m, site_out);
    for (s, _) in sites.sites.iter().zip(&landings).filter(|(_, l)| **l == Landing::Stranded) {
        file.warnings.push(format!(
            "site `{}`: centre on water, no dry routing cell within {} m: routed from the water",
            s.key, recipe.landing_search_m.value
        ));
    }
    let mut crossings_by_kind = BTreeMap::new();
    for c in file.segments.iter().flat_map(|s| &s.crossings) {
        *crossings_by_kind.entry(c.kind.clone()).or_insert(0) += 1;
    }
    let summary = Summary {
        n: frame.n,
        cell_m: frame.cell_m,
        routing_cell_m,
        seconds,
        segments: file.segments.len(),
        km_by_class: file.length_by_class_m.iter().map(|(c, m)| (c.clone(), m / 1000.0)).collect(),
        crossings_by_kind,
        unreachable: file.unreachable.clone(),
    };
    Ok(Run { raster, file, summary })
}

/// One end of an output segment: a routed junction (Magna Urbis's index), or a cut on a site's edge.
enum End {
    Routed(usize),
    Cut(usize, [f64; 2]),
}

/// Magna Urbis's network in the guide frame, cut back to the sites' edges (`ends::clip`): each cut end gets a `site`
/// junction of its own on the edge, a crossing goes with the piece nearest its middle point (dropped when that point is
/// inside a site), routed junctions no segment ends at any more are dropped, and ids are renumbered.
fn convert(out: &RoadOutput, frame: &Frame, recipe: &Recipe, routing_cell_m: f64, sites: Vec<SiteOut>) -> RoadsFile {
    let pts = |path: &[Point]| path.iter().map(|&p| frame.to_guide(p)).collect::<Vec<_>>();
    let extents: Vec<Extent> =
        sites.iter().map(|s| Extent { centre: s.centre_m, landing: s.landing, radius: s.radius_m }).collect();
    let in_site = |p: [f64; 2]| extents.iter().any(|e| std::iter::once(e.centre).chain(e.landing).any(|c| ends::dist(p, c) < e.radius));
    let mut segments: Vec<SegmentOut> = Vec::new();
    let mut seg_ends: Vec<[End; 2]> = Vec::new();
    for s in &out.segments {
        let pieces = ends::clip(&pts(&s.path), &extents);
        let mut crossings: Vec<Vec<CrossingOut>> = vec![Vec::new(); pieces.len()];
        for c in &s.crossings {
            let path = pts(&c.path);
            let Some(&mid) = path.get(path.len() / 2) else { continue };
            let near = (0..pieces.len()).min_by(|&i, &j| {
                ends::distance_to(&pieces[i].points, mid).total_cmp(&ends::distance_to(&pieces[j].points, mid))
            });
            if let (Some(i), false) = (near, in_site(mid)) {
                crossings[i].push(CrossingOut { kind: c.kind.clone(), points: path });
            }
        }
        for (piece, crossings) in pieces.into_iter().zip(crossings) {
            let end = |cut: Option<usize>, routed: usize, p: [f64; 2]| cut.map_or(End::Routed(routed), |site| End::Cut(site, p));
            let (first, last) = (piece.points[0], piece.points[piece.points.len() - 1]);
            seg_ends.push([end(piece.cut[0], s.a, first), end(piece.cut[1], s.b, last)]);
            segments.push(SegmentOut {
                id: segments.len(),
                class: s.class.clone(),
                surface: s.surface.clone(),
                profile: s.profile.clone(),
                width_m: s.width,
                a: 0,
                b: 0,
                length_m: ends::length(&piece.points),
                points: piece.points,
                crossings,
                trimmed_m: piece.trimmed_m,
                grade: s.grade,
                importance: s.importance,
            });
        }
    }
    let mut used = vec![false; out.junctions.len()];
    for e in seg_ends.iter().flatten() {
        if let End::Routed(j) = *e {
            used[j] = true;
        }
    }
    let mut junctions: Vec<JunctionOut> = Vec::new();
    let mut new_id = vec![usize::MAX; out.junctions.len()];
    for (old, j) in out.junctions.iter().enumerate().filter(|(i, _)| used[*i]) {
        let label = j.site.as_deref();
        let site = label.filter(|k| sites.iter().any(|s| s.key == *k));
        let kind = match (site, label) {
            (Some(_), _) => "site",
            (None, Some(other)) => other,
            (None, None) => "junction",
        };
        new_id[old] = junctions.len();
        junctions.push(JunctionOut {
            id: junctions.len(),
            pos: frame.to_guide(j.point),
            degree: 0,
            kind: kind.into(),
            sites: site.iter().map(|k| (*k).to_string()).collect(),
        });
    }
    for (seg, ends) in segments.iter_mut().zip(&seg_ends) {
        let mut id = |e: &End| match *e {
            End::Routed(j) => new_id[j],
            End::Cut(site, pos) => {
                let id = junctions.len();
                junctions.push(JunctionOut { id, pos, degree: 0, kind: "site".into(), sites: vec![sites[site].key.clone()] });
                id
            }
        };
        seg.a = id(&ends[0]);
        seg.b = id(&ends[1]);
    }
    let mut length_by_class_m = BTreeMap::new();
    for s in &segments {
        junctions[s.a].degree += 1;
        junctions[s.b].degree += 1;
        *length_by_class_m.entry(s.class.clone()).or_insert(0.0) += s.length_m;
    }
    let unreachable =
        sites.iter().filter(|s| !junctions.iter().any(|j| j.sites.contains(&s.key))).map(|s| s.key.clone()).collect();
    RoadsFile {
        format: ROADS_FORMAT,
        tool: format!("worldgen-roads {}", env!("CARGO_PKG_VERSION")),
        magna_urbis_rev: MAGNA_URBIS_REV.into(),
        seed: recipe.seed.value.clone(),
        frame: *frame,
        routing_cell_m,
        sites,
        segments,
        junctions,
        warnings: out.warnings.clone(),
        unreachable,
        length_by_class_m,
    }
}

/// `roads.json` text (pretty, trailing newline).
///
/// # Errors
/// Serialization errors.
pub fn to_json(file: &RoadsFile) -> Result<String, String> {
    serde_json::to_string_pretty(file).map(|s| s + "\n").map_err(|e| e.to_string())
}
