// sver.toml ファイルの操作を扱うモジュール
use std::{
    collections::{btree_map::Iter, BTreeMap},
    error::Error,
    fmt::Display,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use git2::{Index, Repository};
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{match_samefile_or_include_dir, SEPARATOR_BYTE};

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub(crate) struct ProfileConfig {
    #[serde(default)]
    pub(crate) excludes: Vec<String>,
    #[serde(default)]
    pub(crate) dependencies: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub(crate) struct SverConfig {
    #[serde(skip)]
    pub(crate) target_path: String,
    #[serde(default, flatten)]
    profiles: BTreeMap<String, ProfileConfig>,
}

impl SverConfig {
    pub(crate) fn get(&self, key: &str) -> Option<ProfileConfig> {
        self.profiles.get(key).cloned()
    }

    pub(crate) fn add(&mut self, profile: &str, config: ProfileConfig) -> Option<ProfileConfig> {
        self.profiles.insert(profile.to_owned(), config)
    }

    pub(crate) fn iter(&self) -> Iter<String, ProfileConfig> {
        self.profiles.iter()
    }

    pub(crate) fn write_initial_config(path: &Path) -> Result<bool, Box<dyn Error>> {
        let mut config = Self::default();
        config.add("default", ProfileConfig::default());

        if File::open(path).is_ok() {
            return Ok(false);
        }

        let mut file = File::create(path)?;
        file.write_all(toml::to_string_pretty(&config)?.as_bytes())?;
        file.flush()?;
        Ok(true)
    }

    fn entry_parent(path: &str) -> Result<String, Box<dyn Error>> {
        let mut path_buf = PathBuf::new();
        path_buf.push(path);
        let result = path_buf.parent().and_then(|path| path.to_str());
        let result = result.map(|s| s.to_string());
        result.ok_or_else(|| "invalid path".into())
    }

    pub(crate) fn config_file_path(&self) -> String {
        if self.target_path.is_empty() {
            "sver.toml".to_owned()
        } else {
            format!("{}/sver.toml", &self.target_path)
        }
    }

    pub(crate) fn load_all_configs(repo: &Repository) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut result: Vec<Self> = Vec::new();
        for entry in repo.index()?.iter() {
            let is_sver_config_in_root_directory = entry.path == "sver.toml".as_bytes();
            let is_sver_config_in_sub_directory = entry
                .path
                .ends_with([SEPARATOR_BYTE, "sver.toml".as_bytes()].concat().as_slice());
            debug!(
                "path:{}, is_root:{}, is_sub:{}",
                String::from_utf8(entry.path.clone())?,
                is_sver_config_in_root_directory,
                is_sver_config_in_sub_directory
            );
            if is_sver_config_in_root_directory || is_sver_config_in_sub_directory {
                debug!("load sver. path:{}", String::from_utf8(entry.path.clone())?);
                let target_path = Self::entry_parent(&String::from_utf8(entry.path.clone())?)?;
                let blob = repo.find_blob(entry.id)?;

                debug!("content:{}", String::from_utf8(blob.content().to_vec())?);

                let mut config = toml::from_slice::<Self>(blob.content())?;
                config.target_path = target_path;
                result.push(config);
            }
        }
        Ok(result)
    }
}

#[derive(Default, Debug)]
struct InnerVerifyResult {
    pub(crate) invalid_excludes: Vec<String>,
    pub(crate) invalid_dependencies: Vec<String>,
}

impl InnerVerifyResult {
    fn is_empty(&self) -> bool {
        self.invalid_dependencies.is_empty() && self.invalid_excludes.is_empty()
    }
}

#[derive(Debug)]
pub(crate) enum VerifyResult {
    Valid {
        path: String,
        profile: String,
    },
    Invalid {
        path: String,
        profile: String,
        invalid_excludes: Vec<String>,
        invalid_dependencies: Vec<String>,
    },
}

impl Display for VerifyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyResult::Valid { path, profile } => {
                writeln!(f, "[OK]\t{}/sver.toml:[{}]", path, profile)
            }
            VerifyResult::Invalid {
                path,
                profile,
                invalid_dependencies,
                invalid_excludes,
            } => {
                writeln!(f, "[NG]\t{}/sver.toml:[{}]", path, profile)?;
                writeln!(f, "\t\tinvalid_dependency:{:?}", invalid_dependencies)?;
                writeln!(f, "\t\tinvalid_exclude:{:?}", invalid_excludes)
            }
        }
    }
}

impl ProfileConfig {
    pub(crate) fn load_profile(
        content: &[u8],
        profile: &str,
    ) -> Result<ProfileConfig, Box<dyn Error>> {
        let config = toml::from_slice::<SverConfig>(content)?;
        config
            .get(profile)
            .ok_or_else(|| format!("profile[{}] is not found", profile).into())
    }

    pub(crate) fn verify(&self, path: &str, profile: &str, index: &Index) -> VerifyResult {
        let mut result = InnerVerifyResult::default();

        result
            .invalid_dependencies
            .extend(self.dependencies.clone());
        result.invalid_excludes.extend(self.excludes.clone());

        for entry in index.iter() {
            result.invalid_dependencies.retain(|dependency| {
                !match_samefile_or_include_dir(&entry.path, dependency.as_bytes())
            });
            result.invalid_excludes.retain(|exclude| {
                let normalized_path = if path.is_empty() {
                    exclude.as_bytes().to_vec()
                } else {
                    [path.as_bytes(), SEPARATOR_BYTE, exclude.as_bytes()].concat()
                };

                let is_match = match_samefile_or_include_dir(&entry.path, &normalized_path);

                debug!(
                    "exclude {}, {}, match:{}",
                    String::from_utf8(entry.path.clone().to_vec()).unwrap(),
                    String::from_utf8(normalized_path).unwrap(),
                    is_match,
                );
                !is_match
            });
        }

        debug!("path:{}, verify_result:{:?}", path, result);

        if result.is_empty() {
            VerifyResult::Valid {
                path: path.to_string(),
                profile: profile.to_string(),
            }
        } else {
            VerifyResult::Invalid {
                path: path.to_string(),
                profile: profile.to_string(),
                invalid_excludes: result.invalid_excludes.clone(),
                invalid_dependencies: result.invalid_dependencies.clone(),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::sver_config::{ProfileConfig, SverConfig};

    #[test]
    fn sver_configs_test() {
        let test = r#"[default]
dependencies = ["dep1"]
excludes = ["exclude1"]
[ext]
dependencies = ["dep2"]
excludes = ["exclude2"]
"#;
        let configs = toml::from_slice::<SverConfig>(test.as_bytes()).unwrap();
        assert_eq!(configs.profiles.len(), 2);
        assert_eq!(
            configs.profiles.keys().cloned().collect::<Vec<String>>(),
            vec!["default", "ext"]
        );
        assert_eq!(
            configs.get("default").unwrap(),
            ProfileConfig {
                dependencies: vec!["dep1".to_owned()],
                excludes: vec!["exclude1".to_owned()],
            }
        );
        assert!(configs.target_path.is_empty());

        let toml_str = toml::to_string_pretty(&configs).unwrap();
        println!("{}", toml_str);
    }
}
