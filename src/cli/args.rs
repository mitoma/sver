use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about = "Version calcurator based on source code.", long_about = None)]
pub(crate) struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// calc version
    Calc {
        /// target paths
        paths: Vec<String>,

        /// format of calucuration result
        #[clap(arg_enum, short, long, default_value = "version-only")]
        output: OutputFormat,
        /// length of version
        #[clap(arg_enum, short, long, default_value = "short")]
        length: VersionLength,
    },
    /// list package dependencies
    List {
        /// target path
        #[clap(default_value = ".")]
        path: String,
    },

    /// generate empty config file
    Init {
        /// target path
        #[clap(default_value = ".")]
        path: String,
    },

    /// validate all config files in repository
    Validate,
}

#[derive(Debug, Clone, clap::ArgEnum)]
pub(crate) enum OutputFormat {
    VersionOnly,
    Toml,
    Json,
}

#[derive(Debug, Clone, clap::ArgEnum)]
pub(crate) enum VersionLength {
    Short,
    Long,
}
