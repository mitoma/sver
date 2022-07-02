pub mod filemode;
mod sver_config;

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    path::{Component, Path, PathBuf},
};

use self::filemode::FileMode;
use self::sver_config::SverConfig;
use git2::{Oid, Repository};
use log::debug;
use sha2::{Digest, Sha256};

pub fn init_sver_config(path: &str) -> Result<String, Box<dyn Error>> {
    debug!("path:{}", path);
    let ResolvePathResult {
        repo, target_path, ..
    } = resolve_target_repo_and_path(path)?;
    let mut path_buf = PathBuf::new();
    path_buf.push(target_path);
    path_buf.push("sver.toml");
    let config_path = path_buf.as_path();

    if repo.index()?.get_path(config_path, 0).is_some() {
        return Ok("sver.toml is already exists".into());
    }
    if !SverConfig::write_initial_config(config_path)? {
        return Ok(format!(
            "sver.toml is already exists. but not commited. path:{}",
            path
        ));
    }
    Ok(format!("sver.toml is generated. path:{}", path))
}

pub fn verify_sver_config() -> Result<(), Box<dyn Error>> {
    let ResolvePathResult { repo, .. } = resolve_target_repo_and_path(".")?;
    let configs = SverConfig::load_all_configs(&repo)?;
    configs.keys().for_each(|key| debug!("{}", key));
    configs.iter().for_each(|(config_file, sver_config)| {
        sver_config.iter().for_each(|(profile, config)| {
            let path = if let Some(parent) = Path::new(config_file).parent() {
                parent.to_str().ok_or("invalid path name").unwrap()
            } else {
                ""
            };

            if let Some(result) = config.verify(path, &repo).unwrap() {
                println!("[NG]\t{}:{}", config_file, profile);
                println!("\tinvalid_dependency:{:?}", result.invalid_dependencies);
                println!("\tinvalid_exclude:{:?}", result.invalid_excludes);
            } else {
                println!("[OK]\t{}:[{}]", config_file, profile,);
            }
        });
    });
    Ok(())
}

pub fn list_sources(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let ResolvePathResult {
        repo, target_path, ..
    } = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let result: Vec<String> = entries
        .iter()
        .map(|(path, _oid)| String::from_utf8(path.clone()).unwrap())
        .collect();
    Ok(result)
}

pub fn calc_version(path: &str) -> Result<Version, Box<dyn Error>> {
    let ResolvePathResult {
        repo,
        work_dir,
        target_path,
    } = resolve_target_repo_and_path(path)?;

    let entries = list_sorted_entries(&repo, &target_path)?;
    let version = calc_hash_string(&repo, target_path.as_bytes(), &entries)?;

    let version = Version {
        repository_root: work_dir,
        path: target_path,
        version,
    };
    Ok(version)
}

struct ResolvePathResult {
    repo: Repository,
    work_dir: String,
    target_path: String,
}

fn resolve_target_repo_and_path(path: &str) -> Result<ResolvePathResult, Box<dyn Error>> {
    let target_path = Path::new(path);
    let repo = find_repository(target_path)?;
    let target_path = relative_path(&repo, target_path)?;
    let target_path = target_path
        .iter()
        .flat_map(|os| os.to_str())
        .collect::<Vec<_>>()
        .join("/");
    let work_dir = repo
        .workdir()
        .and_then(|p| p.to_str())
        .ok_or("bare repository")?
        .to_string();
    debug!("repository_root:{}", work_dir);
    debug!("target_path:{}", target_path);
    Ok(ResolvePathResult {
        repo,
        work_dir,
        target_path,
    })
}

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

fn calc_hash_string(
    repo: &Repository,
    target_path: &[u8],
    source: &BTreeMap<Vec<u8>, OidAndMode>,
) -> Result<String, Box<dyn Error>> {
    let mut hasher = Sha256::default();
    hasher.update(target_path);
    for (path, oid_and_mode) in source {
        hasher.update(path);
        match oid_and_mode.mode {
            FileMode::Blob | FileMode::BlobExecutable | FileMode::Link => {
                // Q. Why little endian?
                // A. no reason.
                hasher.update(u32::from(oid_and_mode.mode).to_le_bytes());
                let blob = repo.find_blob(oid_and_mode.oid)?;
                let content = blob.content();
                hasher.update(content);
                debug!(
                    "path:{}, mode:{:?}, content:{}",
                    String::from_utf8(path.clone())?,
                    oid_and_mode.mode,
                    String::from_utf8(content.to_vec())?
                )
            }
            // Commit (Submodule の場合は参照先のコミットハッシュを計算対象に加える)
            FileMode::Commit => {
                debug!("commit_hash?:{}", oid_and_mode.oid);
                hasher.update(oid_and_mode.oid);
            }
            _ => {
                debug!(
                    "unsupported mode. skipped. path:{}, mode:{:?}",
                    String::from_utf8(path.clone())?,
                    oid_and_mode.mode
                )
            }
        }
    }
    let hash = format!("{:#x}", hasher.finalize());
    Ok(hash)
}

fn list_sorted_entries(
    repo: &Repository,
    path_str: &str,
) -> Result<BTreeMap<Vec<u8>, OidAndMode>, Box<dyn Error>> {
    let mut path_set: HashMap<String, Vec<String>> = HashMap::new();
    collect_path_and_excludes(repo, path_str, &mut path_set)?;
    debug!("dependency_paths:{:?}", path_set);
    let mut map = BTreeMap::new();
    for entry in repo.index()?.iter() {
        let containable = containable(entry.path.as_slice(), &path_set);
        debug!(
            "path:{}, containable:{}, mode:{:?}",
            String::from_utf8(entry.path.clone())?,
            containable,
            FileMode::from(entry.mode),
        );
        if containable {
            debug!("add path:{:?}", String::from_utf8(entry.path.clone()));
            map.insert(
                entry.path,
                OidAndMode {
                    oid: entry.id,
                    mode: entry.mode.into(),
                },
            );
        }
    }
    Ok(map)
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

fn collect_path_and_excludes(
    repo: &Repository,
    path: &str,
    path_and_excludes: &mut HashMap<String, Vec<String>>,
) -> Result<(), Box<dyn Error>> {
    if path_and_excludes.contains_key(path) {
        debug!("already added. path:{}", path.to_string());
        return Ok(());
    }
    debug!("add dep path : {}", path);

    let mut p = PathBuf::new();
    p.push(path);
    p.push("sver.toml");

    let mut current_path_and_excludes: HashMap<String, Vec<String>> = HashMap::new();

    if let Some(entry) = repo.index()?.get_path(p.as_path(), 0) {
        debug!("sver.toml exists. path:{:?}", String::from_utf8(entry.path));
        let default_config =
            SverConfig::load_profile(repo.find_blob(entry.id)?.content(), "default")?;
        current_path_and_excludes.insert(path.to_string(), default_config.excludes.clone());
        path_and_excludes.insert(path.to_string(), default_config.excludes);
        for dependency_path in default_config.dependencies {
            collect_path_and_excludes(repo, &dependency_path, path_and_excludes)?;
        }
    } else {
        current_path_and_excludes.insert(path.to_string(), vec![]);
        path_and_excludes.insert(path.to_string(), vec![]);
    }

    // include synbolic link
    for entry in repo.index()?.iter() {
        let containable = containable(entry.path.as_slice(), &current_path_and_excludes);
        if containable && FileMode::Link == FileMode::from(entry.mode) {
            let path = String::from_utf8(entry.path)?;
            let mut buf = PathBuf::new();
            buf.push(path);
            buf.pop();

            let blob = repo.find_blob(entry.id)?;
            let link_path = String::from_utf8(blob.content().to_vec())?;
            let link_path = Path::new(&link_path);
            for link_components in link_path.components() {
                debug!("link_component:{:?}", link_components);
                match link_components {
                    Component::ParentDir => {
                        buf.pop();
                    }
                    Component::Normal(path) => buf.push(path),
                    Component::RootDir => {}
                    Component::CurDir => {}
                    Component::Prefix(_prefix) => {}
                }
            }

            let link_path = buf
                .to_str()
                .ok_or("path is invalid")?
                .replace(OS_SEP_STR, SEPARATOR_STR);
            debug!("collect link path. path:{}", &link_path);
            collect_path_and_excludes(repo, &link_path, path_and_excludes)?;
        }
    }
    Ok(())
}

fn find_repository(from_path: &Path) -> Result<Repository, Box<dyn Error>> {
    for target_path in from_path.canonicalize()?.ancestors() {
        if let Ok(repo) = Repository::open(target_path) {
            return Ok(repo);
        }
    }
    Err("repository was not found".into())
}
