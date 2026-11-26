//! `worldgen-roads --guide <dir> --sites <sites.json> --recipe <toml> --out <roads.json> [--settled <key>=<streets.json>]...`.
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// Route the regional road network between fixed settlements on a Scrib guide with Magna Urbis.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Guide folder (format 1: meta.json, heights.f32; optional water.json).
    #[arg(long)]
    guide: PathBuf,
    /// Settlements to link (sites.json, format 1, guide frame metres).
    #[arg(long)]
    sites: PathBuf,
    /// Recipe (TOML, every value with its unit and source).
    #[arg(long)]
    recipe: PathBuf,
    /// Output file (roads.json).
    #[arg(long)]
    out: PathBuf,
    /// A laid-out site's settled streets (`settle`'s streets.json): its regional roads are cut where they meet them.
    #[arg(long, value_name = "KEY=STREETS_JSON", value_parser = key_path)]
    settled: Vec<(String, PathBuf)>,
}

/// `<key>=<path>`, split at the first `=`.
fn key_path(arg: &str) -> Result<(String, PathBuf), String> {
    match arg.split_once('=') {
        Some((key, path)) if !key.is_empty() && !path.is_empty() => Ok((key.to_string(), PathBuf::from(path))),
        _ => Err(format!("`{arg}`: want <key>=<streets.json>")),
    }
}

fn run(args: &Args) -> Result<String, String> {
    let at = |e: String| format!("{}: {e}", args.recipe.display());
    let text = std::fs::read_to_string(&args.recipe).map_err(|e| at(e.to_string()))?;
    let recipe = roads::Recipe::parse(&text).map_err(at)?;
    let sites = roads::SitesFile::load(&args.sites)?;
    let settled =
        args.settled.iter().map(|(key, path)| roads::Settled::load(key, path)).collect::<Result<Vec<_>, _>>()?;
    let run = roads::route(&args.guide, &sites, &recipe, &settled)?;
    let json = roads::run::to_json(&run.file)?;
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&args.out, json).map_err(|e| format!("{}: {e}", args.out.display()))?;
    for w in &run.file.warnings {
        eprintln!("warning: {w}");
    }
    Ok(format!("{} -> {}", run.summary, args.out.display()))
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(line) => {
            println!("worldgen-roads: {line}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worldgen-roads: {e}");
            ExitCode::FAILURE
        }
    }
}
