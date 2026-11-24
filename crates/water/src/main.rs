//! `worldgen-water --guide <guide dir> --recipe <recipe.toml> --out <dir>`.
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// Run Magna Urbis's water chain on a guide folder: carved guide folder plus water.json (lakes, rivers).
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Guide folder (format 1: meta.json, heights.f32, optional stamps.json + stamps.f32, anything else copied).
    #[arg(long)]
    guide: PathBuf,
    /// Recipe (TOML, every value with its unit and source).
    #[arg(long)]
    recipe: PathBuf,
    /// Output folder (a new guide folder).
    #[arg(long)]
    out: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let result = std::fs::read_to_string(&args.recipe)
        .map_err(|e| e.to_string())
        .and_then(|text| water::Recipe::parse(&text).map(|r| (r, text)))
        .map_err(|e| format!("{}: {e}", args.recipe.display()))
        .and_then(|(recipe, text)| water::run(&args.guide, &recipe, &text, &args.out));
    match result {
        Ok(report) => {
            println!("{} -> {}", report.summary(), args.out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worldgen-water: {e}");
            ExitCode::FAILURE
        }
    }
}
