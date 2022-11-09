mod cli;

use std::process::ExitCode;

use crate::cli::outputs::format_versions;

use self::cli::args::{Args, Commands, OutputFormat, VersionLength};
use clap::Parser;
use log::debug;
use sver::{sver_repository::SverRepository, Version};

fn main() -> ExitCode {
    env_logger::init();
    let args = Args::parse();

    let result = match args.command {
        Commands::Calc {
            paths,
            output,
            length,
        } => calc(paths, output, length),
        Commands::List { path } => list(&path),
        Commands::Init { path } => init(&path),
        Commands::Validate => validate(),
    };
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            println!("{}", e);
            ExitCode::FAILURE
        }
    }
}

fn calc(paths: Vec<String>, output: OutputFormat, length: VersionLength) -> anyhow::Result<()> {
    let paths = if paths.is_empty() {
        vec![".".to_string()]
    } else {
        paths
    };
    debug!("paths:{:?}", paths);
    let versions = paths
        .iter()
        .map(|p| SverRepository::new(p)?.calc_version())
        .collect::<anyhow::Result<Vec<Version>>>()?;
    println!("{}", format_versions(&versions, output, length)?);
    Ok(())
}

fn list(path: &str) -> anyhow::Result<()> {
    SverRepository::new(path)?
        .list_sources()?
        .iter()
        .for_each(|s| println!("{}", s));
    Ok(())
}

fn init(path: &str) -> anyhow::Result<()> {
    println!("{}", SverRepository::new(path)?.init_sver_config()?);
    Ok(())
}

fn validate() -> anyhow::Result<()> {
    SverRepository::new(".")?
        .validate_sver_config()?
        .iter()
        .for_each(|s| print!("{}", s));
    Ok(())
}
