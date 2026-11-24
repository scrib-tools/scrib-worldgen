//! A data pack adds a culture "like an existing one" (`crate::data`): the culture, its
//! presets and its mapping are `$from` copies, and it joins its source's family lists with
//! `$append`. The culture checks of the catalogue must then hold for it.
//!
//! Packs install once per process, before the first read, so the checks run in a child
//! process of this test binary (`child_cloned_culture`, ignored unless the parent sets
//! `CHILD`).
use serde_json::{Value, json};

const CHILD: &str = "MAGNA_URBIS_CULTURE_PACK_CHILD";
const SOURCE: &str = "mediterranean-modern";
const CLONE: &str = "pack-clone";
const PRESETS: [&str; 5] = ["core", "ring", "faubourg", "village", "hamlet"];

fn pack() -> Value {
    let presets: serde_json::Map<String, Value> = PRESETS
        .iter()
        .map(|p| {
            let id = format!("{CLONE}/{p}");
            (id.clone(), json!({ "$from": format!("/presets/{SOURCE}~1{p}"), "params": { "id": id } }))
        })
        .collect();
    // The culture's recipes point at its own presets (arrays replace as a whole).
    let recipes: Vec<Value> = [("core", "core"), ("ring", "middle"), ("faubourg", "faubourg"), ("village", "village"), ("hamlet", "village")]
        .iter()
        .map(|(id, zone)| json!({ "id": id, "label": id, "zone": zone, "preset": format!("{CLONE}/{id}") }))
        .collect();
    let append = json!({ "$append": [CLONE] });
    json!({
        "catalogue": {
            "presets": presets,
            "cultures": { CLONE: { "$from": format!("/cultures/{SOURCE}"), "label": "Pack clone", "recipes": recipes } },
            "farmstead": { "cultures": { CLONE: { "$from": format!("/farmstead/cultures/{SOURCE}") } } }
        },
        "culture_mappings": { "cultures": { CLONE: { "$from": format!("/cultures/{SOURCE}") } } },
        "activities": { "cultures": { "families": { "industrial": { "cultures": append } } } },
        "enclosures": { "groups": { "openStreetClosedBack": { "cultures": append } } },
        "harbours": { "families": { "contemporary": { "cultures": append } } },
        "services": { "cultures": { CLONE: "industrial" } },
        "styles_modern": {
            "palettes": { CLONE: { "$from": format!("/palettes/{SOURCE}") } },
            "cultures": { CLONE: { "$from": format!("/cultures/{SOURCE}") } }
        }
    })
}

#[test]
fn data_pack_adds_a_culture_cloned_from_another() {
    if std::env::var_os(CHILD).is_some() {
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["urban::culture_pack_tests::child_cloned_culture", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{text}");
    assert!(text.contains("1 passed"), "the child test did not run:\n{text}");
}

#[test]
#[ignore = "child of data_pack_adds_a_culture_cloned_from_another"]
fn child_cloned_culture() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    crate::data::install_packs(&[pack()]).unwrap();
    let cat = super::catalogue();
    let culture = &cat["cultures"][CLONE];
    assert_eq!(culture["plan"], cat["cultures"][SOURCE]["plan"]);
    // profile and morphology of every recipe (profiles.rs, culture.rs checks)
    let p = super::profiles::profile(CLONE);
    assert_eq!(p.pattern, super::profiles::profile(SOURCE).pattern);
    for r in culture["recipes"].as_array().unwrap() {
        let preset = r["preset"].as_str().unwrap();
        assert_eq!(cat["presets"][preset]["params"]["id"], json!(preset));
        for stage in ["quarters", "parcels"] {
            super::resolve(CLONE, "culture", r["id"].as_str().unwrap(), r["zone"].as_str().unwrap(), 1.0, &json!({}), stage)
                .unwrap_or_else(|e| panic!("{preset}: {e}"));
        }
    }
    // explicit architecture route, the same as its source's
    let (route, source) = (super::culture::mapping(CLONE).expect("route"), super::culture::mapping(SOURCE).unwrap());
    assert_eq!(route.culture, CLONE);
    assert_eq!(
        (&route.architecture_pack, &route.admissibility.recipe_id, &route.admissibility.native_layout),
        (&source.architecture_pack, &source.admissibility.recipe_id, &source.admissibility.native_layout)
    );
    assert!(route.admissibility.min_area > 0.0);
    assert_eq!(super::culture::mappings().len(), cat["cultures"].as_object().unwrap().len());
    // family lists (enclosure.rs, activities.rs checks), services, styles
    let listed = |doc: &str, groups: &str| {
        crate::data::doc(doc).pointer(groups).unwrap().as_object().unwrap().values().any(|g| g["cultures"].as_array().unwrap().iter().any(|x| x == CLONE))
    };
    assert!(listed("enclosures", "/groups"));
    assert!(listed("activities", "/cultures/families"));
    assert!(listed("harbours", "/families"));
    assert_eq!(super::roles::family(CLONE), super::roles::family(SOURCE));
    assert_eq!(super::forms::kit::house_parameters(CLONE), super::forms::kit::house_parameters(SOURCE));
    // a small town end to end (culture.rs `every_catalogue_culture_generates_a_small_town`)
    use super::{LayoutConfig, ParcelConfig, Point, Site, Street, Surface};
    let n = 64;
    let (height, water, level) = (vec![20.0; n * n], vec![0; n * n], vec![0.0; n * n]);
    let surface = Surface { geometry: None, width: 1600.0, n, height: &height, water: &water, interpolation: Default::default() };
    let road = |a: (f64, f64), b: (f64, f64)| Street {
        path: vec![Point { x: a.0, y: a.1 }, Point { x: b.0, y: b.1 }],
        widths: vec![6.0, 6.0],
        rank: 1,
        role: "regional".into(),
    };
    let roads = [road((100.0, 800.0), (1500.0, 800.0)), road((800.0, 100.0), (800.0, 1500.0))];
    let cfg = LayoutConfig { footprint: super::config::legacy_footprint(), ..LayoutConfig::default() };
    let site = Site {
        key: "town".into(),
        name: "Town".into(),
        population: 1500,
        culture: CLONE.into(),
        center: Point { x: 800.0, y: 800.0 },
        radius: 320.0,
        neighbours: Vec::new(),
    };
    let seed = format!("culture-town-{CLONE}");
    let layouts = super::generate_quarters(&seed, &[site], &cfg, surface, &roads).unwrap();
    let partitions = super::generate_parcels(&seed, &layouts, &ParcelConfig::default()).unwrap();
    let sites = super::building_sites::requests(&seed, &layouts, &partitions, surface, &level, None).unwrap();
    assert!(!layouts[0].blocks.is_empty() && !partitions[0].parcels.is_empty() && !sites.is_empty());
}
