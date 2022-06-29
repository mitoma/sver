// sver.toml ファイルの操作を扱うモジュール
use std::{collections::BTreeMap, error::Error, fs::File, io::Write, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct SverConfig {
    #[serde(default)]
    pub(crate) excludes: Vec<String>,
    #[serde(default)]
    pub(crate) dependencies: Vec<String>,
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
}
