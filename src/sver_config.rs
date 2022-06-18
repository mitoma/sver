use std::{collections::BTreeMap, error::Error};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
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
}
