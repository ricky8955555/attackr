use std::sync::LazyLock;
use time::{PrimitiveDateTime, UtcOffset};

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

fn default_name() -> String {
    "attackr".to_string()
}

fn default_timezone() -> UtcOffset {
    UtcOffset::UTC
}

fn validate_config(config: &Config) -> Result<(), ValidationError> {
    if config.start_at >= config.end_at {
        return Err(ValidationError::new(
            "'start_at' must be earlier than 'end_at'.",
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_config"))]
pub struct Config {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_timezone")]
    pub timezone: UtcOffset,
    #[serde(default)]
    pub start_at: Option<PrimitiveDateTime>,
    #[serde(default)]
    pub end_at: Option<PrimitiveDateTime>,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| super::load_config("event"));
