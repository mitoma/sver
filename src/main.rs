use std::error::Error;

use clap::Parser;
use log::debug;
use serde::Serialize;
use sver::{calc_version, Version};

fn main() {
    env_logger::init();
    let args = Args::parse();
    std::process::exit(match run(args) {
        Ok(_) => 0,
        Err(e) => {
            println!("{}", e);
            1
        }
    })
}

#[derive(Parser, Debug)]
#[clap(author, version, about = "version calucurator for git repository", long_about = None)]
struct Args {
    /// sver paths
    paths: Vec<String>,

    #[clap(arg_enum, short, long, default_value = "version-only")]
    output: OutputFormat,
    #[clap(arg_enum, short, long, default_value = "short")]
    length: VersionLength,
}

#[derive(Debug, Clone, clap::ArgEnum)]
enum OutputFormat {
    VersionOnly,
    Toml,
    Json,
}

#[derive(Debug, Clone, clap::ArgEnum)]
enum VersionLength {
    Short,
    Long,
}

#[derive(Serialize)]
struct VersionOutput {
    repository_root: String,
    path: String,
    version: String,
}

#[derive(Serialize)]
struct VersionsOutput {
    versions: Vec<VersionOutput>,
}

#[derive(Serialize)]
struct VersionFullOutput {
    repository_root: String,
    path: String,
    short_version: String,
    long_version: String,
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let paths = if args.paths.is_empty() {
        vec![".".to_string()]
    } else {
        args.paths
    };
    debug!("paths:{:?}", paths);
    let versions = paths
        .iter()
        .map(|p| crate::calc_version(p))
        .collect::<Result<Vec<Version>, Box<dyn Error>>>()?;
    print_versions(&versions, args.output, args.length)?;
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
