//! `settle --site <site dir> --recipe <recipe.toml> --out <dir> [--kit <kit.json>] [--roads <roads.json>]`.
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// Grow or found a settlement on a site folder with Magna Urbis; one layout folder per recipe culture.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Site folder (format 1: site.json, heights.f32, water.u8, optional roads.u8).
    #[arg(long)]
    site: PathBuf,
    /// Recipe (TOML, every value with its unit and source).
    #[arg(long)]
    recipe: PathBuf,
    /// Output folder; gets one subfolder per culture.
    #[arg(long)]
    out: PathBuf,
    /// Kit (`kit.json`) to fit buildings from; overrides the recipe's `fit.kit`. Needs the recipe's `[fit]` table.
    #[arg(long)]
    kit: Option<PathBuf>,
    /// The guide's regional roads (`roads.json` from `worldgen-roads`, format 1, guide frame): the town is laid along
    /// the segments on the site's crop instead of the recipe's founding crossing.
    #[arg(long)]
    roads: Option<PathBuf>,
}

fn run(args: &Args) -> Result<Vec<serde_json::Value>, String> {
    let at = |e: String| format!("{}: {e}", args.recipe.display());
    let text = std::fs::read_to_string(&args.recipe).map_err(|e| at(e.to_string()))?;
    let recipe = settle::Recipe::parse(&text).map_err(at)?;
    let name = args.recipe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    // A recipe's kit and culture pack paths are relative to the recipe file.
    let recipe_dir = args.recipe.parent().unwrap_or_else(|| std::path::Path::new(""));
    // Before any Magna Urbis call: its data is fixed at the first read.
    settle::culture_packs::install(&settle::culture_packs::paths(&recipe, recipe_dir))?;
    let kit_path = args.kit.clone().or_else(|| recipe.fit.as_ref().and_then(|f| f.kit.as_ref()).map(|k| recipe_dir.join(k)));
    let kit = kit_path.map(|p| settle::KitSource::load(&p, &name)).transpose()?;
    let roads = args.roads.as_deref().map(settle::RegionalRoads::load).transpose()?;
    settle::settle_with(&args.site, &recipe, &args.out, kit.as_ref(), roads.as_ref())
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(stats) => {
            if let Some(by_role) = stats.first().map(|s| &s["regional_roads_by_role"]).and_then(|r| r.as_object()) {
                let roles: Vec<String> = by_role.iter().map(|(role, n)| format!("{n} {role}")).collect();
                let total: u64 = by_role.values().filter_map(serde_json::Value::as_u64).sum();
                println!("regional roads: {total} streets kept on the crop ({})", roles.join(", "));
            }
            for s in stats {
                println!(
                    "{}: {} lots ({} with a footprint, requests {} ms), {} streets ({} m), built {} m2, {:.2} s -> {}",
                    s["culture"].as_str().unwrap_or("?"),
                    s["lots"],
                    s["lots_with_preview"],
                    s["requests_ms"],
                    s["streets"],
                    s["street_length_m"],
                    s["built_area_m2"]["new"],
                    s["run_time_s"]["total"].as_f64().unwrap_or(0.0),
                    args.out.join(settle::run::culture_dir(s["culture"].as_str().unwrap_or(""))).display(),
                );
                let x = &s["streets_export"];
                if !x.is_null() {
                    println!(
                        "  streets.json: {} segments, {} junctions, km by class {}, by surface {}, crossings {}, {} regional, max grade {}",
                        x["segments"], x["junctions"], x["km_by_class"], x["km_by_surface"], x["crossings_by_kind"], x["regional"], x["max_grade"],
                    );
                }
                let f = &s["fit"];
                if !f.is_null() {
                    println!(
                        "  fit: {} lots tried, {} filled, empty {}, {} instances, {} models, {} ms",
                        f["lots_tried"], f["lots_filled"], f["lots_empty_by_reason"], f["instances"], f["distinct_models"], f["fit_ms"],
                    );
                    if f["domain"] == "match" {
                        println!(
                            "  fit: match {} of {} building parts (tried {}; by stage {}), {} of {} plot lots with one, parts empty {}, IoU p10/p50/p90 {}, left alone {}",
                            f["parts_filled"],
                            f["building_parts"],
                            f["parts_tried"],
                            f["parts_filled_by_stage"],
                            f["lots_filled"],
                            f["lots_tried"],
                            f["parts_empty_by_reason"],
                            f["match_iou_p10_p50_p90"],
                            f["parts_skipped_by_label"],
                        );
                    }
                }
                let g = &s["gardens"];
                if !g.is_null() {
                    println!(
                        "  gardens (temporary scaffold): {} features, {} m2, by source {}, outer ring only {}",
                        g["features"], g["area_m2"], g["by_source"], g["outer_ring_only"],
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("settle: {e}");
            ExitCode::FAILURE
        }
    }
}
