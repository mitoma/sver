use std::{
    collections::{BTreeMap, HashMap},
    path::{Component, Path, PathBuf},
};

use anyhow::Context;
use git2::Repository;
use log::{debug, log_enabled, Level};
use sha2::{Digest, Sha256};

use crate::{
    containable,
    filemode::FileMode,
    find_repository, relative_path,
    sver_config::{CalculationTarget, ProfileConfig, SverConfig, ValidationResult},
    OidAndMode, Version, SEPARATOR_STR,
};

pub struct SverRepository {
    repo: Repository,
    work_dir: String,
    calculation_target: CalculationTarget,
}

impl SverRepository {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let calculation_target = CalculationTarget::parse(path);

        let target_path = Path::new(&calculation_target.path);
        let repo = find_repository(target_path)?;
        let target_path = relative_path(&repo, target_path)?;
        let target_path = target_path
            .iter()
            .flat_map(|os| os.to_str())
            .collect::<Vec<_>>()
            .join(SEPARATOR_STR);
        let work_dir = repo
            .workdir()
            .and_then(|p| p.to_str())
            .with_context(|| "bare repository")?
            .to_string();
        debug!("repository_root:{}", work_dir);
        debug!("target_path:{}", target_path);

        let calculation_target = CalculationTarget::new(target_path, calculation_target.profile);
        Ok(Self {
            repo,
            work_dir,
            calculation_target,
        })
    }

    pub fn init_sver_config(&self) -> anyhow::Result<String> {
        debug!("path:{}", self.calculation_target.path);
        let mut path_buf = PathBuf::new();
        path_buf.push(&self.calculation_target.path);
        path_buf.push("sver.toml");
        let config_path = path_buf.as_path();

        if self.repo.index()?.get_path(config_path, 0).is_some() {
            return Ok("sver.toml already exists".into());
        }

        let mut fs_path = PathBuf::new();
        fs_path.push(&self.work_dir);
        fs_path.push(config_path);
        if !SverConfig::write_initial_config(fs_path.as_path())? {
            return Ok(format!(
                "sver.toml already exists, but is not committed. path:{}",
                self.calculation_target.path
            ));
        }
        Ok(format!(
            "sver.toml is generated. path:{}",
            self.calculation_target.path
        ))
    }

    pub fn validate_sver_config(&self) -> anyhow::Result<Vec<ValidationResult>> {
        let configs = SverConfig::load_all_configs(&self.repo)?;
        if log_enabled!(Level::Debug) {
            configs
                .iter()
                .for_each(|config| debug!("{}", config.config_file_path()));
        }
        let index = self.repo.index()?;
        let result: Vec<ValidationResult> = configs
            .iter()
            .flat_map(|sver_config| {
                let target_path = sver_config.target_path.clone();
                sver_config
                    .iter()
                    .map(|(profile, config)| {
                        config.validate(&target_path, profile, &index, &self.repo)
                    })
                    .collect::<Vec<ValidationResult>>()
            })
            .collect();
        Ok(result)
    }

    pub fn list_sources(&self) -> anyhow::Result<Vec<String>> {
        let entries = self.list_sorted_entries()?;
        let result = entries
            .iter()
            .map(|(path, _oid)| String::from_utf8(path.clone()).unwrap())
            .collect();
        Ok(result)
    }

    pub fn calc_version(&self) -> anyhow::Result<Version> {
        let entries = self.list_sorted_entries()?;
        let version = self.calc_hash_string(&entries)?;

        let version = Version {
            repository_root: self.work_dir.clone(),
            path: self.calculation_target.path.clone(),
            version,
        };
        Ok(version)
    }

    fn calc_hash_string(&self, source: &BTreeMap<Vec<u8>, OidAndMode>) -> anyhow::Result<String> {
        let mut hasher = Sha256::default();
        hasher.update(self.calculation_target.path.as_bytes());
        for (path, oid_and_mode) in source {
            hasher.update(path);
            match oid_and_mode.mode {
                FileMode::Blob | FileMode::BlobExecutable | FileMode::Link => {
                    // Q. Why little endian?
                    // A. no reason.
                    hasher.update(u32::from(oid_and_mode.mode).to_le_bytes());
                    hasher.update(oid_and_mode.oid);
                    debug!(
                        "path:{}, mode:{:?}, oid:{}",
                        String::from_utf8(path.clone())?,
                        oid_and_mode.mode,
                        oid_and_mode.oid
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

    fn list_sorted_entries(&self) -> anyhow::Result<BTreeMap<Vec<u8>, OidAndMode>> {
        let mut path_set: HashMap<CalculationTarget, Vec<String>> = HashMap::new();
        self.collect_path_and_excludes(&self.calculation_target, &mut path_set)?;
        debug!("dependency_paths:{:?}", path_set);
        let mut map = BTreeMap::new();
        for entry in self.repo.index()?.iter() {
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

    fn collect_path_and_excludes(
        &self,
        calculation_target: &CalculationTarget,
        path_and_excludes: &mut HashMap<CalculationTarget, Vec<String>>,
    ) -> anyhow::Result<()> {
        if path_and_excludes.contains_key(calculation_target) {
            debug!(
                "already added. path:{}, profile:{}",
                calculation_target.path, calculation_target.profile
            );
            return Ok(());
        }
        debug!("add dep path : {}", calculation_target.path);

        let mut p = PathBuf::new();
        p.push(&calculation_target.path);
        p.push("sver.toml");

        let mut current_path_and_excludes: HashMap<CalculationTarget, Vec<String>> = HashMap::new();

        if let Some(entry) = self.repo.index()?.get_path(p.as_path(), 0) {
            debug!("sver.toml exists. path:{:?}", String::from_utf8(entry.path));
            let config = ProfileConfig::load_profile(
                self.repo.find_blob(entry.id)?.content(),
                &calculation_target.profile,
            )?;
            current_path_and_excludes.insert(calculation_target.clone(), config.excludes.clone());
            path_and_excludes.insert(calculation_target.clone(), config.excludes);
            for dependency in config.dependencies {
                let dependency_target = CalculationTarget::parse_from_setting(&dependency);
                self.collect_path_and_excludes(&dependency_target, path_and_excludes)?;
            }
        } else {
            current_path_and_excludes.insert(calculation_target.clone(), vec![]);
            path_and_excludes.insert(calculation_target.clone(), vec![]);
        }

        // include symbolic link
        for entry in self.repo.index()?.iter() {
            if FileMode::from(entry.mode) == FileMode::Link
                && containable(entry.path.as_slice(), &current_path_and_excludes)
            {
                let path = String::from_utf8(entry.path)?;
                let mut buf = PathBuf::new();
                buf.push(path);
                buf.pop();

                let blob = self.repo.find_blob(entry.id)?;
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
                    .iter()
                    .flat_map(|os| os.to_str())
                    .collect::<Vec<_>>()
                    .join(SEPARATOR_STR);
                debug!("collect link path. path:{}", &link_path);
                self.collect_path_and_excludes(
                    &CalculationTarget::new(link_path, "default".to_string()),
                    path_and_excludes,
                )?;
            }
        }
        Ok(())
    }
}
