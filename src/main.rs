use anyhow::Result;
use clap::Parser;

mod cli;
mod engine;
mod tui;
mod tunnels;

fn main() -> Result<()> {
    engine::telemetry::check_first_run();
    let cli = cli::args::Cli::parse();
    cli::dispatch(cli)
}
