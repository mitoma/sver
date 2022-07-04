mod cli;

use std::{error::Error, process::ExitCode};

use crate::cli::outputs::format_versions;

use self::cli::args::{Args, Commands, OutputFormat, VersionLength};
use clap::Parser;
use log::debug;
use sver::{calc_version, init_sver_config, list_sources, verify_sver_config, Version};

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
        Commands::Verify => verify(),
    };
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            println!("{}", e);
            ExitCode::FAILURE
        }
    }
}

fn calc(
    paths: Vec<String>,
    output: OutputFormat,
    length: VersionLength,
) -> Result<(), Box<dyn Error>> {
    let paths = if paths.is_empty() {
        vec![".".to_string()]
    } else {
        paths
    };
    debug!("paths:{:?}", paths);
    let versions = paths
        .iter()
        .map(|p| crate::calc_version(p))
        .collect::<Result<Vec<Version>, Box<dyn Error>>>()?;
    println!("{}", format_versions(&versions, output, length)?);
    Ok(())
}

fn list(path: &str) -> Result<(), Box<dyn Error>> {
    list_sources(path)?.iter().for_each(|s| println!("{}", s));
    Ok(())
}

fn init(path: &str) -> Result<(), Box<dyn Error>> {
    println!("{}", init_sver_config(path)?);
    Ok(())
}

fn verify() -> Result<(), Box<dyn Error>> {
    verify_sver_config()?.iter().for_each(|s| println!("{}", s));
    Ok(())
}
