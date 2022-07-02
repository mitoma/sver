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

        #[clap(arg_enum, short, long, default_value = "version-only")]
        output: OutputFormat,
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

    /// verify all config files in repository
    Verify,
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
