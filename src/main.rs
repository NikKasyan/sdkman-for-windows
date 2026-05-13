mod api;
mod archive;
mod cli;
mod commands;
mod config;
mod envfile;
mod fslink;
mod shims;
mod state;

use anyhow::Result;
use clap::Parser;

fn main() {
    if let Err(error) = run() {
        eprintln!("Stop! {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = cli::Args::parse();
    let state = state::State::load()?;
    commands::execute(args, state)
}
