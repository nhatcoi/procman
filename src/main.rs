use anyhow::Result;
use clap::Parser;

mod cli;
mod engine;
mod tui;
mod tunnels;

fn main() -> Result<()> {
    let cli = cli::args::Cli::parse();
    cli::dispatch(cli)
}
