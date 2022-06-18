mod cli;

use std::{error::Error, process::ExitCode};

use crate::cli::outputs::print_versions;

use self::cli::args::{Args, Commands, OutputFormat, VersionLength};
use clap::Parser;
use log::debug;
use sver::{calc_version, list_sources, Version};

fn main() -> ExitCode {
    env_logger::init();
    let args = Args::parse();

    let result = match args.command {
        Commands::Calc {
            paths,
            output,
            length,
        } => calc(paths, output, length),
        Commands::List { path } => list(path),
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
    print_versions(&versions, output, length)?;
    Ok(())
}

fn list(path: String) -> Result<(), Box<dyn Error>> {
    list_sources(&path)?.iter().for_each(|s| println!("{}", s));
    Ok(())
}
