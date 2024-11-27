use anyhow::{anyhow, bail, Result};
use async_tempfile::{TempDir, TempFile};
use bollard::{
    container::{
        Config as ContainerConfig, RemoveContainerOptions, StartContainerOptions,
        WaitContainerOptions,
    },
    image::{BuildImageOptions, BuilderVersion, CreateImageOptions, RemoveImageOptions},
    secret::{BuildInfoAux, HostConfig, PortBinding},
    Docker,
};
use flate2::{write::GzEncoder, Compression};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
    ops::RangeInclusive,
    path::Path,
};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
};
use validator::Validate;

use crate::utils::fsext;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CmdStep {
    #[validate(length(min = 1))]
    pub image: String,
    #[validate(length(min = 1))]
    pub cmds: Vec<String>,
    #[serde(default)]
    pub envs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DockerStep {
    #[validate(length(min = 1))]
    pub path: String,
    #[serde(default)]
    pub config: DockerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Step {
    Cmd(CmdStep),
    Docker(DockerStep),
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BinaryArtifactInfo {
    pub name: Option<String>,
    #[validate(length(min = 1))]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArtifactInfo {
    Binary(BinaryArtifactInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryArtifact {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerConfig {
    #[serde(default)]
    pub exposed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerArtifact {
    pub id: String,
    pub config: DockerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Artifact {
    Binary(BinaryArtifact),
    Docker(DockerArtifact),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BuildInfo {
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDockerResult {
    pub id: String,
    pub ports: HashMap<String, u16>,
}

fn default_addrs() -> Vec<IpAddr> {
    vec![
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DockerRunOptions {
    #[validate(range(exclusive_min = 0.0))]
    #[serde(default)]
    pub cpus: Option<f64>,
    #[validate(range(min = 1))]
    #[serde(default)]
    pub memory: Option<i64>,
    #[serde(default)]
    pub storage: Option<String>,
    #[serde(default = "default_addrs")]
    pub addrs: Vec<IpAddr>,
    #[serde(default)]
    pub ports: Option<RangeInclusive<u16>>,
}

impl Default for DockerRunOptions {
    fn default() -> Self {
        Self {
            cpus: Default::default(),
            memory: Default::default(),
            storage: Default::default(),
            addrs: default_addrs(),
            ports: Default::default(),
        }
    }
}

pub async fn load_build_info<P: AsRef<Path>>(path: P) -> Result<BuildInfo> {
    let path = path.as_ref().join("build.yml");
    let mut fp = File::open(path).await?;

    let mut yaml = String::new();
    fp.read_to_string(&mut yaml).await?;

    let build: BuildInfo =
        tokio::task::spawn_blocking(move || serde_yml::from_str(&yaml)).await??;
    build.validate()?;

    Ok(build)
}

async fn execute_cmd_step<P: AsRef<Path>>(path: P, flag: &str, step: &CmdStep) -> Result<()> {
    let docker = Docker::connect_with_defaults()?;

    let options = CreateImageOptions::<&str> {
        from_image: &step.image,
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(options), None, None);
    let mut info = None;

    while let Some(Ok(inner)) = stream.next().await {
        info = Some(inner)
    }

    let info = info.ok_or_else(|| anyhow!("no response from create_image"))?;

    if let Some(err) = info.error {
        bail!("pull image failed: {err}");
    }

    let mut script = TempFile::new().await?;
    let content = step.cmds.join("\n");
    script.write_all(content.as_bytes()).await?;

    let source_path = path
        .as_ref()
        .to_str()
        .ok_or_else(|| anyhow!("inconvertible path."))?;
    let working_dir = "/src";
    let source_bind = format!("{source_path}:{working_dir}");

    let script_path = script
        .file_path()
        .to_str()
        .ok_or_else(|| anyhow!("inconvertible path."))?;
    let dest_script_path = "/build.sh";
    let script_bind = format!("{script_path}:{dest_script_path}");

    let host_config = HostConfig {
        binds: Some(vec![source_bind, script_bind]),
        ..Default::default()
    };

    let env = format!("ATTACKR_FLAG={flag}");

    let entrypoint = vec!["/bin/sh", dest_script_path];

    let config: ContainerConfig<&str> = ContainerConfig {
        image: Some(&step.image),
        host_config: Some(host_config),
        cmd: Some(entrypoint),
        env: Some(vec![&env]),
        working_dir: Some(working_dir),
        ..Default::default()
    };

    let created = docker.create_container::<&str, _>(None, config).await?;

    let result = async {
        docker
            .start_container(&created.id, None::<StartContainerOptions<&str>>)
            .await?;

        let mut stream = docker.wait_container(&created.id, None::<WaitContainerOptions<&str>>);
        let mut resp = None;

        while let Some(Ok(inner)) = stream.next().await {
            resp = Some(inner);
        }

        if let Some(err) = resp.and_then(|resp| resp.error) {
            bail!("wait container error: {}", err.message.unwrap_or_default());
        }

        let inspect = docker.inspect_container(&created.id, None).await?;

        if let Some(state) = inspect.state {
            if let Some(exit_code) = state.exit_code {
                if exit_code != 0 {
                    bail!(
                        "exited with code {exit_code}: {}",
                        state.error.unwrap_or_default()
                    )
                }
            }
        }

        Ok(())
    }
    .await;

    let options = RemoveContainerOptions {
        force: true,
        ..Default::default()
    };

    docker.remove_container(&created.id, Some(options)).await?;

    result
}

async fn process_binary_artifact<P, Q>(
    path: P,
    target: Q,
    artifact: &BinaryArtifactInfo,
) -> Result<BinaryArtifact>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let source = path.as_ref().join(&artifact.path);

    let name: Option<&str> = match artifact.name.as_ref().filter(|name| name.is_empty()) {
        Some(val) => Some(val),
        None => source.file_name().and_then(|name| name.to_str()),
    };
    let name = name.ok_or_else(|| anyhow!("no file name available."))?;

    let target = target.as_ref().join(name);

    if target.exists() {
        bail!("File or directory {target:?} already exists");
    }

    if source.is_dir() {
        let copy_options = fsext::CopyOptions::new();
        fsext::copy_dir(&source, &target, &copy_options).await?;
    } else {
        fs::copy(&source, &target).await?;
    }

    Ok(BinaryArtifact {
        path: name.to_string(),
    })
}

async fn execute_docker_step<P: AsRef<Path>>(
    step: &DockerStep,
    path: P,
    flag: &str,
) -> Result<DockerArtifact> {
    let docker = Docker::connect_with_defaults()?;

    let mut tarfile = Vec::new();
    let path = path.as_ref().join(&step.path);

    {
        let enc = GzEncoder::new(&mut tarfile, Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all("", &path)?;
        tar.finish()?;
    }

    let buildargs = HashMap::from([("ATTACKR_FLAG", flag)]);

    let name = uuid::Uuid::new_v4().as_simple().to_string();

    let options = BuildImageOptions {
        buildargs,
        t: &name,
        dockerfile: "Dockerfile",
        session: Some(name.clone()),
        version: BuilderVersion::BuilderBuildKit,
        ..Default::default()
    };

    let mut stream = docker.build_image(options, None, Some(tarfile.into()));

    let mut result = None;

    while let Some(Ok(bollard::models::BuildInfo { aux: Some(aux), .. })) = stream.next().await {
        result = Some(aux);
    }

    if let Some(result) = result {
        match result {
            BuildInfoAux::BuildKit(resp) => {
                let error = resp
                    .vertexes
                    .into_iter()
                    .last()
                    .map(|vertex| vertex.error)
                    .unwrap_or_default();

                bail!("build image failed: {error}");
            }
            BuildInfoAux::Default(inner) => {
                let id = inner.id.ok_or_else(|| anyhow!("no image id got."))?;
                return Ok(DockerArtifact {
                    id,
                    config: step.config.clone(),
                });
            }
        }
    }

    bail!("no response from build_image.");
}

fn is_port_free(addr: IpAddr, port: u16) -> bool {
    let addr = SocketAddr::new(addr, port);
    TcpListener::bind(addr).is_ok()
}

pub async fn run_docker(
    artifact: &DockerArtifact,
    options: &DockerRunOptions,
) -> Result<RunDockerResult> {
    options.validate()?;

    let ports: HashMap<_, _> = artifact
        .config
        .exposed
        .iter()
        .cloned()
        .zip(
            options
                .ports
                .clone()
                .unwrap_or(1..=65535u16)
                .filter(|port| options.addrs.iter().all(|addr| is_port_free(*addr, *port))),
        )
        .collect();

    let docker = Docker::connect_with_defaults()?;

    let port_bindings = ports
        .clone()
        .into_iter()
        .map(|(exposed, port)| {
            (
                exposed,
                Some(
                    options
                        .addrs
                        .iter()
                        .map(|addr| PortBinding {
                            host_ip: Some(addr.to_string()),
                            host_port: Some(port.to_string()),
                        })
                        .collect(),
                ),
            )
        })
        .collect();

    let storage_opt = options
        .storage
        .clone()
        .map(|size| HashMap::from([("size".to_string(), size)]));

    let host_config = HostConfig {
        port_bindings: Some(port_bindings),
        publish_all_ports: Some(false),
        cpu_quota: options.cpus.map(|x| (x * 100000.0).round() as i64),
        memory: options.memory,
        storage_opt,
        ..Default::default()
    };

    let config = ContainerConfig {
        image: Some(artifact.id.clone()),
        host_config: Some(host_config),
        ..Default::default()
    };

    let created = docker.create_container::<&str, _>(None, config).await?;

    docker
        .start_container(&created.id, None::<StartContainerOptions<&str>>)
        .await?;

    Ok(RunDockerResult {
        id: created.id,
        ports,
    })
}

pub async fn stop_docker(id: &str) -> Result<()> {
    let docker = Docker::connect_with_defaults()?;

    let options = RemoveContainerOptions {
        force: true,
        ..Default::default()
    };

    docker.remove_container(id, Some(options)).await?;

    Ok(())
}

async fn remove_docker_artifact(artifact: &DockerArtifact) -> Result<()> {
    let docker = Docker::connect_with_defaults()?;

    let options = RemoveImageOptions {
        force: true,
        ..Default::default()
    };

    docker
        .remove_image(&artifact.id, Some(options), None)
        .await?;

    Ok(())
}

pub async fn clear_artifact<P: AsRef<Path>>(path: P, artifacts: &[Artifact]) {
    if let Err(e) = fs::remove_dir_all(&path).await {
        log::error!(target: "conductor", "failed to remove dir: {e:?}")
    }

    for artifact in artifacts {
        #[allow(clippy::single_match)]
        match artifact {
            Artifact::Docker(artifact) => {
                if let Err(e) = remove_docker_artifact(artifact).await {
                    log::error!(target: "conductor", "failed to remove docker artifact: {e:?}")
                }
            }
            _ => {}
        }
    }
}

pub async fn build<P, Q>(source: P, target: Q, erase: bool, flag: &str) -> Result<BuildResult>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let tempdir = TempDir::new().await?;

    if target.as_ref().exists() {
        if erase {
            fs::remove_dir_all(&target).await?;
        } else {
            bail!("File exists: {:?}", target.as_ref());
        }
    }

    fs::create_dir_all(&target).await?;

    let result = async {
        let copy_options = fsext::CopyOptions::new();
        fsext::copy_dir(&source, &tempdir, &copy_options).await?;

        let build = load_build_info(&tempdir).await?;

        let mut artifacts = Vec::new();

        for step in &build.steps {
            match step {
                Step::Cmd(step) => execute_cmd_step(&tempdir, flag, step).await?,
                Step::Docker(step) => artifacts.push(Artifact::Docker(
                    execute_docker_step(step, &tempdir, flag).await?,
                )),
            };
        }

        for artifact in &build.artifacts {
            #[allow(clippy::single_match)]
            match artifact {
                ArtifactInfo::Binary(artifact) => artifacts.push(Artifact::Binary(
                    process_binary_artifact(&tempdir, &target, artifact).await?,
                )),
            }
        }

        Ok(BuildResult { artifacts })
    }
    .await;

    if result.is_err() {
        _ = fs::remove_dir_all(&target).await;
    }

    _ = fs::remove_dir_all(&tempdir).await;

    result
}
