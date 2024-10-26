use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::RangeInclusive,
    path::PathBuf,
    sync::LazyLock,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

use crate::core::conductor::DockerRunOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedAddr {
    pub addr: IpAddr,
    #[serde(default)]
    pub ports: Option<RangeInclusive<u16>>,
}

fn default_expiry() -> Option<Duration> {
    Some(Duration::from_secs(30 * 60))
}

fn validate_docker_config(config: &DockerConfig) -> Result<(), ValidationError> {
    for addr in &config.mapped_addrs {
        if let Some(mapped_ports) = &addr.ports {
            if let Some(ports) = &config.options.ports {
                if ports.len() != mapped_ports.len() {
                    return Err(ValidationError::new(
                        "'mapped_ports' must be the same length as 'options.ports'",
                    ));
                }
            } else {
                return Err(ValidationError::new(
                    "'mapped_ports' is set without 'options.port' set",
                ));
            }
        }
    }

    Ok(())
}

fn default_mapped_addrs() -> Vec<MappedAddr> {
    vec![
        MappedAddr {
            addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
            ports: None,
        },
        MappedAddr {
            addr: IpAddr::V6(Ipv6Addr::LOCALHOST),
            ports: None,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_docker_config"))]
pub struct DockerConfig {
    #[serde(default = "default_mapped_addrs")]
    pub mapped_addrs: Vec<MappedAddr>,
    #[serde(default = "default_expiry")]
    pub expiry: Option<Duration>,
    #[serde(default)]
    #[validate(nested)]
    pub options: DockerRunOptions,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            mapped_addrs: default_mapped_addrs(),
            expiry: default_expiry(),
            options: Default::default(),
        }
    }
}

fn default_challenge_root() -> PathBuf {
    "challenges".into()
}

fn default_artifact_root() -> PathBuf {
    "artifacts".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Config {
    #[serde(default = "default_challenge_root")]
    pub challenge_root: PathBuf,
    #[serde(default = "default_artifact_root")]
    pub artifact_root: PathBuf,
    #[serde(default)]
    #[validate(nested)]
    pub docker: DockerConfig,
    #[serde(default)]
    pub dynpoints: Option<PathBuf>,
    #[serde(default)]
    pub clear_on_solved: bool,
    #[serde(default)]
    pub show_uncategorized: bool,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| super::load_config("challenge"));
