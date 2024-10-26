use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use std::time::Duration;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Key {
    KeyPair { privkey: String, pubkey: String },
    Passphrase { passphrase: String },
}

fn default_expiry() -> Duration {
    Duration::from_secs(12 * 60 * 60)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JWTConfig {
    pub algo: jsonwebtoken::Algorithm,
    pub key: Key,
    #[serde(default = "default_expiry")]
    pub expiry: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Config {
    pub jwt: JWTConfig,
    #[serde(default)]
    pub no_verify: bool,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| super::load_config("user"));
