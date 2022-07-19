pub mod filemode;
pub mod sver_config;
pub mod sver_repository;

use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
};

use self::filemode::FileMode;
use git2::{Oid, Repository};

pub struct Version {
    pub repository_root: String,
    pub path: String,
    pub version: String,
}

fn relative_path(repo: &Repository, path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let repo_path = repo
        .workdir()
        .and_then(|p| p.canonicalize().ok())
        .ok_or("bare repository is not supported")?;
    let current_path = path.canonicalize()?;
    let result = current_path.strip_prefix(repo_path)?.to_path_buf();
    Ok(result)
}

struct OidAndMode {
    oid: Oid,
    mode: FileMode,
}

#[cfg(target_os = "windows")]
const OS_SEP_STR: &str = "\\";
#[cfg(target_os = "linux")]
const OS_SEP_STR: &str = "/";

const SEPARATOR_STR: &str = "/";
const SEPARATOR_BYTE: &[u8] = SEPARATOR_STR.as_bytes();

fn containable(test_path: &[u8], path_set: &HashMap<String, Vec<String>>) -> bool {
    path_set.iter().any(|(include, excludes)| {
        let include_file = match_samefile_or_include_dir(test_path, include.as_bytes());
        let exclude_file = excludes.iter().any(|exclude| {
            if include.is_empty() {
                match_samefile_or_include_dir(test_path, exclude.as_bytes())
            } else {
                match_samefile_or_include_dir(
                    test_path,
                    [include.as_bytes(), SEPARATOR_BYTE, exclude.as_bytes()]
                        .concat()
                        .as_slice(),
                )
            }
        });
        include_file && !exclude_file
    })
}

fn match_samefile_or_include_dir(test_path: &[u8], path: &[u8]) -> bool {
    let is_same_file = test_path == path;
    let is_contain_path =
        path.is_empty() || test_path.starts_with([path, SEPARATOR_BYTE].concat().as_slice());
    is_same_file || is_contain_path
}

fn find_repository(from_path: &Path) -> Result<Repository, Box<dyn Error>> {
    for target_path in from_path.canonicalize()?.ancestors() {
        if let Ok(repo) = Repository::open(target_path) {
            return Ok(repo);
        }
    }
    Err("repository was not found".into())
}
