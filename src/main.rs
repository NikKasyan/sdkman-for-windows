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
use std::ffi::OsString;

fn main() {
    if let Err(error) = run() {
        eprintln!("Stop! {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let raw_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let args = match cli::Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let exit_code = error.exit_code();
            error.print()?;
            if let Some(examples) = examples_for_raw_args(&raw_args) {
                eprintln!();
                eprintln!("{examples}");
            }
            std::process::exit(exit_code);
        }
    };
    let state = state::State::load()?;
    commands::execute(args, state)
}

fn examples_for_raw_args(args: &[OsString]) -> Option<&'static str> {
    let command = args
        .iter()
        .filter_map(|arg| arg.to_str())
        .find(|arg| !arg.starts_with('-'))?;
    let subcommand = args
        .iter()
        .filter_map(|arg| arg.to_str())
        .skip_while(|arg| *arg != command)
        .skip(1)
        .find(|arg| !arg.starts_with('-'));
    cli::examples_for(command, subcommand)
}
