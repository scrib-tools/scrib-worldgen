//! Culture packs (`settle::culture_packs`): TOML pack files parse to JSON and install into Magna Urbis, whose
//! readers then see the merged data. Packs install once per process: this test binary holds the one install.
use std::path::PathBuf;

/// A tiny pack: a clone of `mediterranean-modern` with one site preference of its own.
const TINY: &str = r#"
[catalogue.cultures.pack-test]
"$from" = "/cultures/mediterranean-modern"
label = "Pack test"
sitePrefs = { packTestKey = 7.5 }
"#;

fn temp_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("settle-culture-packs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_tiny_pack_and_the_hlaalu_pack_install_into_magna_urbis() {
    let tiny = temp_file("tiny.toml", TINY);
    let json = settle::culture_packs::load(&tiny).unwrap();
    assert_eq!(json["catalogue"]["cultures"]["pack-test"]["$from"], "/cultures/mediterranean-modern");
    assert_eq!(json["catalogue"]["cultures"]["pack-test"]["sitePrefs"]["packTestKey"], 7.5);

    let hlaalu = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../recipes/cultures/hlaalu.toml");
    settle::culture_packs::install(&[tiny.clone(), hlaalu]).unwrap();
    assert!((magna_urbis_core::urban::culture_site_pref("pack-test", "packTestKey", 0.0) - 7.5).abs() < 1e-12);
    let (route, source) = (
        magna_urbis_core::urban::culture::mapping("hlaalu").expect("hlaalu route"),
        magna_urbis_core::urban::culture::mapping("mediterranean-modern").unwrap(),
    );
    assert_eq!(route.architecture_pack, source.architecture_pack);
    assert!(magna_urbis_core::urban::planned_land("hlaalu", 12000).is_some());

    // Data is fixed now: a second install is refused, and the message names the file.
    let err = settle::culture_packs::install(std::slice::from_ref(&tiny)).unwrap_err();
    assert!(err.contains("tiny.toml"), "{err}");
}

#[test]
fn a_pack_that_is_not_toml_names_its_file() {
    let bad = temp_file("bad.toml", "[catalogue\n");
    let err = settle::culture_packs::load(&bad).unwrap_err();
    assert!(err.contains("bad.toml"), "{err}");
}
