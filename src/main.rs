mod derive;
mod grid;
mod input;
mod render;
mod search;

use std::{fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use clap::Parser;
use derive::derive_candidates;
use input::PuzzleInput;
use render::render_puzzles;
use search::{SearchConfig, search_best_n};

#[derive(Parser, Debug)]
#[command(name = "crossword-generator")]
#[command(about = "Generate dense song-linked crossword layouts from YAML input")]
struct Cli {
    #[arg(long, default_value = "input.yaml")]
    input: PathBuf,
    #[arg(long, default_value_t = 5_000)]
    time_limit_ms: u64,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value_t = 24)]
    branch_limit: usize,
    #[arg(long, default_value_t = 1)]
    top_n: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let yaml = fs::read_to_string(&cli.input)
        .with_context(|| format!("failed to read {}", cli.input.display()))?;
    let puzzle: PuzzleInput = serde_yaml::from_str(&yaml)
        .with_context(|| format!("failed to parse {}", cli.input.display()))?;
    puzzle.validate()?;

    let candidates = derive_candidates(&puzzle)?;
    if candidates.is_empty() {
        anyhow::bail!("no viable answer candidates were derived from the input");
    }

    let config = SearchConfig {
        time_limit: Duration::from_millis(cli.time_limit_ms),
        seed: cli.seed,
        branch_limit: cli.branch_limit,
        top_n: cli.top_n.max(1),
    };

    let solutions = search_best_n(&puzzle, &candidates, &config)
        .context("search did not produce any valid crossword candidate")?;
    let output = render_puzzles(&solutions, &config);
    println!("{output}");

    Ok(())
}
