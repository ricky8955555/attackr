use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use validator::Validate;

fn default_expiry() -> Duration {
    Duration::from_secs(12 * 60 * 60)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_expiry")]
    pub expiry: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            expiry: default_expiry(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Config {
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub no_verify: bool,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| super::load_config("user"));
