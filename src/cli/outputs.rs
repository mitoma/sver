use serde::Serialize;
use sver::Version;

use super::args::{OutputFormat, VersionLength};

#[derive(Serialize)]
struct VersionOutput {
    pub(crate) repository_root: String,
    pub(crate) path: String,
    pub(crate) version: String,
}

#[derive(Serialize)]
struct VersionsOutput {
    pub(crate) versions: Vec<VersionOutput>,
}

pub(crate) fn format_versions(
    versions: &[Version],
    output_format: OutputFormat,
    version_length: VersionLength,
) -> anyhow::Result<String> {
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
    Ok(output_string)
}
