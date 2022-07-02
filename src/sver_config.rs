// sver.toml ファイルの操作を扱うモジュール
use std::{collections::BTreeMap, error::Error, fs::File, io::Write, path::Path};

use git2::Repository;
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{match_samefile_or_include_dir, SEPARATOR_BYTE};

#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct SverConfig {
    #[serde(default)]
    pub(crate) excludes: Vec<String>,
    #[serde(default)]
    pub(crate) dependencies: Vec<String>,
}

#[derive(Default, Debug)]
pub(crate) struct VerifyResult {
    pub(crate) invalid_excludes: Vec<String>,
    pub(crate) invalid_dependencies: Vec<String>,
}

impl VerifyResult {
    fn is_empty(&self) -> bool {
        self.invalid_dependencies.is_empty() && self.invalid_excludes.is_empty()
    }
}

impl SverConfig {
    pub(crate) fn load_profile(
        content: &[u8],
        profile: &str,
    ) -> Result<SverConfig, Box<dyn Error>> {
        let config = toml::from_slice::<BTreeMap<String, SverConfig>>(content)?;
        Ok(config[profile].clone())
    }

    pub(crate) fn write_initial_config(path: &Path) -> Result<bool, Box<dyn Error>> {
        let mut config = BTreeMap::new();
        config.insert("default", SverConfig::default());

        if File::open(path).is_ok() {
            return Ok(false);
        }

        let mut file = File::create(path)?;
        file.write_all(toml::to_string_pretty(&config)?.as_bytes())?;
        file.flush()?;
        Ok(true)
    }

    pub(crate) fn load_all_configs(
        repo: &Repository,
    ) -> Result<BTreeMap<String, BTreeMap<String, SverConfig>>, Box<dyn Error>> {
        let mut result = BTreeMap::new();
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

                let blob = repo.find_blob(entry.id)?;

                debug!("content:{}", String::from_utf8(blob.content().to_vec())?);

                let config = toml::from_slice::<BTreeMap<String, SverConfig>>(blob.content())?;
                result.insert(String::from_utf8(entry.path)?, config);
            }
        }
        Ok(result)
    }

    pub(crate) fn verify(
        &self,
        path: &str,
        repo: &Repository,
    ) -> Result<Option<VerifyResult>, Box<dyn Error>> {
        let mut result = VerifyResult::default();

        result
            .invalid_dependencies
            .extend(self.dependencies.clone());
        result.invalid_excludes.extend(self.excludes.clone());

        for entry in repo.index()?.iter() {
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
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}
