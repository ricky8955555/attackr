use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::activity::ActivityKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub path: String,
    pub kinds: Vec<ActivityKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Config {
    #[serde(default)]
    pub scripts: Vec<Script>,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| super::load_config("activity"));
