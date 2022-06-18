mod cli;

use std::{error::Error, process::ExitCode};

use crate::cli::outputs::{VersionOutput, VersionsOutput};

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

fn print_versions(
    versions: &[Version],
    output_format: OutputFormat,
    version_length: VersionLength,
) -> Result<(), Box<dyn Error>> {
    let output: Vec<VersionOutput> = versions
        .iter()
        .map(|v| {
            let mut version_string = v.version.clone();
            match version_length {
                VersionLength::Short => version_string.truncate(12),
                VersionLength::Long => (),
            };
            VersionOutput {
                repository_root: v.repository_root.clone(),
                path: v.path.clone(),
                version: version_string,
            }
        })
        .collect();

    let output_string = match output_format {
        OutputFormat::VersionOnly => {
            let out = output
                .iter()
                .map(|o| &o.version)
                .cloned()
                .collect::<Vec<String>>()
                .join("\n");
            out
        }
        OutputFormat::Toml => {
            if output.len() == 1 {
                toml::to_string(&output[0])?
            } else {
                toml::to_string(&VersionsOutput { versions: output })?
            }
        }
        OutputFormat::Json => {
            if output.len() == 1 {
                serde_json::to_string_pretty(&output[0])?
            } else {
                serde_json::to_string_pretty(&output)?
            }
        }
    };
    println!("{}", output_string);
    Ok(())
}
