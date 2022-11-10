pub mod filemode;
pub mod sver_config;
pub mod sver_repository;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use self::filemode::FileMode;
use anyhow::{anyhow, Context};
use git2::{Oid, Repository};
use sver_config::CalculationTarget;

pub struct Version {
    pub repository_root: String,
    pub path: String,
    pub version: String,
}

fn relative_path(repo: &Repository, path: &Path) -> anyhow::Result<PathBuf> {
    let repo_path = repo
        .workdir()
        .and_then(|p| p.canonicalize().ok())
        .with_context(|| "bare repository is not supported")?;
    let current_path = path.canonicalize()?;
    let result = current_path.strip_prefix(repo_path)?.to_path_buf();
    Ok(result)
}

struct OidAndMode {
    oid: Oid,
    mode: FileMode,
}

const SEPARATOR_STR: &str = "/";
const SEPARATOR_BYTE: &[u8] = SEPARATOR_STR.as_bytes();

fn containable(test_path: &[u8], path_set: &HashMap<CalculationTarget, Vec<String>>) -> bool {
    path_set.iter().any(|(include, excludes)| {
        let include_file = match_samefile_or_include_dir(test_path, include.path.as_bytes());
        let exclude_file = excludes.iter().any(|exclude| {
            if include.path.is_empty() {
                match_samefile_or_include_dir(test_path, exclude.as_bytes())
            } else {
                match_samefile_or_include_dir(
                    test_path,
                    [include.path.as_bytes(), SEPARATOR_BYTE, exclude.as_bytes()]
                        .concat()
                        .as_slice(),
                )
            }
        });
        include_file && !exclude_file
    })
}

fn match_samefile_or_include_dir(test_path: &[u8], path: &[u8]) -> bool {
    is_samefile(test_path, path) || is_contain_path(test_path, path)
}

fn is_samefile(test_path: &[u8], path: &[u8]) -> bool {
    test_path == path
}

fn is_contain_path(test_path: &[u8], path: &[u8]) -> bool {
    path.is_empty() || test_path.starts_with([path, SEPARATOR_BYTE].concat().as_slice())
}

fn find_repository(from_path: &Path) -> anyhow::Result<Repository> {
    for target_path in from_path.canonicalize()?.ancestors() {
        if let Ok(repo) = Repository::open(target_path) {
            return Ok(repo);
        }
    }
    Err(anyhow!("repository was not found"))
}
