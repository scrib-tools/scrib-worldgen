//! `settle --site <site dir> --recipe <recipe.toml> --out <dir>`.
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
}

fn main() -> ExitCode {
    let args = Args::parse();
    let result = std::fs::read_to_string(&args.recipe)
        .map_err(|e| format!("{}: {e}", args.recipe.display()))
        .and_then(|text| settle::Recipe::parse(&text).map_err(|e| format!("{}: {e}", args.recipe.display())))
        .and_then(|recipe| settle::settle(&args.site, &recipe, &args.out));
    match result {
        Ok(stats) => {
            for s in stats {
                println!(
                    "{}: {} lots, {} streets ({} m), built {} m2, {:.2} s -> {}",
                    s["culture"].as_str().unwrap_or("?"),
                    s["lots"],
                    s["streets"],
                    s["street_length_m"],
                    s["built_area_m2"]["new"],
                    s["run_time_s"]["total"].as_f64().unwrap_or(0.0),
                    args.out.join(settle::run::culture_dir(s["culture"].as_str().unwrap_or(""))).display(),
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("settle: {e}");
            ExitCode::FAILURE
        }
    }
}
