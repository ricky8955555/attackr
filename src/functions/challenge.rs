use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    net::SocketAddr,
    path::PathBuf,
    sync::LazyLock,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Result};
use either::Either;
use koto::prelude::*;
use rocket::{fairing::AdHoc, fs::TempFile, Build, Rocket};
use tokio::{
    fs,
    sync::{Mutex, RwLock},
};

use crate::{
    configs::challenge::{MappedAddr, CONFIG},
    core::conductor::{self, Artifact, BuildInfo, RunDockerResult},
    db::{
        models::{Artifact as ArtifactEntry, Challenge, Score, Solved, Submission},
        query::{
            artifact::{
                delete_artifact, delete_dynamic_artifact, get_artifact, list_challenge_artifacts,
                update_artifact,
            },
            challenge::{delete_challenge, get_challenge, list_challenges, update_challenge},
            scores::add_score,
            solved::{list_challenge_effective_solved_with_submission, update_solved},
            submission::add_submission,
        },
        Db,
    },
    utils::{dynfmt, responder::NamedFile, script::KotoScript},
};

use super::event::primitive_now;

const CHECK_CYCLE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct DockerInstance {
    info: RunDockerResult,
    stop_at: Option<Instant>,
}

type ArtifactIndex = (i32, i32, usize);

static DOCKER_INSTANCES: LazyLock<RwLock<HashMap<ArtifactIndex, DockerInstance>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[allow(clippy::type_complexity)]
static BUILDING: LazyLock<RwLock<HashSet<(Option<i32>, i32)>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

static DOCKER_PREPARING: LazyLock<RwLock<HashSet<ArtifactIndex>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

static DYNPOINTS_INSTANCE: LazyLock<Option<Mutex<KotoScript>>> = LazyLock::new(|| {
    if let Some(path) = &CONFIG.dynpoints {
        let mut buf = Vec::new();
        let mut file = File::open(path).expect("open file");
        file.read_to_end(&mut buf).expect("read file");

        let code = String::from_utf8_lossy(&buf);
        let script = KotoScript::compile(&code).expect("compile failed.");

        return Some(Mutex::new(script));
    }

    None
});

async fn docker_instance_check() {
    loop {
        let now = Instant::now();
        let mut expired = Vec::new();

        {
            let instances = DOCKER_INSTANCES.read().await;

            for (key, instance) in instances.iter() {
                if let Some(stop_at) = instance.stop_at {
                    if now >= stop_at {
                        expired.push(*key);
                    }
                }
            }
        }

        for (user, challenge, artifact) in expired {
            _ = stop_docker(user, challenge, artifact).await;
        }

        tokio::time::sleep(CHECK_CYCLE).await;
    }
}

pub async fn initialize(rocket: Rocket<Build>) -> Rocket<Build> {
    tokio::spawn(docker_instance_check());

    rocket
}

pub async fn uninitialize() {
    _ = stop_all_active_sessions().await;
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Function - Challenge", |rocket| async {
        rocket
            .attach(AdHoc::on_ignite(
                "Initialize Challenge Function",
                initialize,
            ))
            .attach(AdHoc::on_liftoff("Uninitialize Challenge Function", |_| {
                Box::pin(async move {
                    uninitialize().await;
                })
            }))
    })
}

fn generate_random_flag(fmt: &str) -> Result<String> {
    let inner = uuid::Uuid::new_v4().hyphenated().to_string();
    dynfmt::format(fmt, &[&inner])
}

async fn calculate_points(initial: f64, solved: i64) -> Result<f64> {
    if let Some(lock) = &*DYNPOINTS_INSTANCE {
        let mut script = lock.lock().await;

        let args = [
            KValue::Number(initial.into()),
            KValue::Number(solved.into()),
        ];

        let ret = tokio::task::spawn_blocking(move || {
            script.call_function("calculate_points", CallArgs::Separate(&args))
        })
        .await??;

        if let KValue::Number(number) = ret {
            let result = match number {
                KNumber::F64(val) => val,
                KNumber::I64(val) => val as f64,
            };

            return Ok(result);
        }

        bail!("unexpected return value '{ret:?}'.");
    }

    Ok(initial)
}

async fn calculate_factor(raw: f64, solved: i64) -> Result<f64> {
    // solved=0 for the first challenger solved.

    if let Some(lock) = &*DYNPOINTS_INSTANCE {
        let mut script = lock.lock().await;

        let args = [KValue::Number(raw.into()), KValue::Number(solved.into())];

        let ret = tokio::task::spawn_blocking(move || {
            script.call_function("calculate_factor", CallArgs::Separate(&args))
        })
        .await??;

        if let KValue::Number(number) = ret {
            let result = match number {
                KNumber::F64(val) => val,
                KNumber::I64(val) => val as f64,
            };

            return Ok(result);
        }

        bail!("unexpected return value '{ret:?}'.");
    }

    Ok(raw)
}

async fn recalculate_challenge_points_consumed(db: &Db, mut challenge: Challenge) -> Result<()> {
    let mut solved =
        list_challenge_effective_solved_with_submission(db, challenge.id.unwrap()).await?;

    solved.sort_unstable_by_key(|x| x.1.time);

    let now = primitive_now();

    let points = calculate_points(challenge.initial, solved.len() as i64).await?;
    challenge.points = points;

    for (idx, data) in solved.into_iter().enumerate() {
        let factor = calculate_factor(challenge.initial, idx as i64).await?;
        let value = points * factor;

        let score = Score {
            id: None,
            user: data.1.user,
            challenge: data.1.challenge,
            time: now,
            points: value,
        };

        let score = add_score(db, score).await?;

        let entry = Solved {
            id: data.0.id,
            submission: data.1.id.unwrap(),
            score: Some(score),
        };

        update_solved(db, entry).await?;
    }

    update_challenge(db, challenge).await?;

    Ok(())
}

pub async fn recalculate_challenge_points(db: &Db, challenge: i32) -> Result<()> {
    let challenge = get_challenge(db, challenge).await?;
    recalculate_challenge_points_consumed(db, challenge).await?;

    Ok(())
}

pub async fn recalculate_points(db: &Db) -> Result<()> {
    let challenges = list_challenges(db).await?;

    for challenge in challenges {
        recalculate_challenge_points_consumed(db, challenge).await?;
    }

    Ok(())
}

pub async fn save_files(
    source: Option<TempFile<'_>>,
    attachments: Vec<TempFile<'_>>,
) -> Result<(String, Vec<String>)> {
    let name = uuid::Uuid::new_v4().hyphenated().to_string();
    let path = CONFIG.challenge_root.join(&name);

    let result = async {
        let attachment_dir = path.join("attachment");

        let mut saved_attachments = Vec::new();

        fs::create_dir_all(&attachment_dir).await?;

        for mut attachment in attachments {
            let file_name = attachment
                .raw_name()
                .ok_or_else(|| anyhow!("attachment name not found."))?;

            let raw_name = file_name.dangerous_unsafe_unsanitized_raw().as_str();
            let santized = file_name.as_str();

            if santized.is_none() {
                bail!("unsafe name detected.");
            }

            let path = PathBuf::from(raw_name);

            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("invalid filename."))?;

            let path = attachment_dir.join(name);

            attachment.move_copy_to(path).await?;
            saved_attachments.push(name.to_string());
        }

        if let Some(source) = source {
            let source = match source {
                TempFile::File { path, .. } => Either::Left(std::fs::File::open(path)?),
                TempFile::Buffered { content } => Either::Right(content),
            };

            let source_dir = path.join("source");

            let mut tar = tar::Archive::new(source);
            tar.unpack(&source_dir)?;

            _ = conductor::load_build_info(&source_dir).await?;
        }

        Ok((name, saved_attachments))
    }
    .await;

    if result.is_err() {
        _ = fs::remove_dir_all(&path).await;
    }

    result
}

pub async fn load_build_info(db: &Db, id: i32) -> Result<BuildInfo> {
    let challenge = get_challenge(db, id).await?;

    let path = CONFIG.challenge_root.join(&challenge.path);
    let source = path.join("source");

    conductor::load_build_info(source).await
}

pub async fn clear_artifact(artifact: &ArtifactEntry) -> bool {
    let stop_sessions_success = stop_active_sessions(artifact.user, artifact.challenge).await;

    let path = CONFIG.artifact_root.join(&artifact.path);
    let clear_artifact_success = conductor::clear_artifact(&path, &artifact.info).await;

    stop_sessions_success && clear_artifact_success
}

pub async fn remove_challenge(db: &Db, id: i32) -> Result<()> {
    let artifacts = list_challenge_artifacts(db, id).await?;

    for artifact in artifacts {
        _ = clear_artifact(&artifact).await;
    }

    let challenge = get_challenge(db, id).await?;

    let path = CONFIG.challenge_root.join(&challenge.path);
    fs::remove_dir_all(&path).await?;

    delete_challenge(db, id).await?;

    Ok(())
}

pub async fn build_challenge(db: &Db, user: Option<i32>, challenge: i32) -> Result<()> {
    if !BUILDING.write().await.insert((user, challenge)) {
        bail!("challenge {challenge} build task for user {user:?} has already started.");
    }

    let result = async {
        let entry = get_challenge(db, challenge).await?;

        if entry.dynamic && user.is_none() {
            bail!("user id is needed for dynamic challenge.");
        }

        if !entry.dynamic && user.is_some() {
            bail!("static challenge should not have a user id assigned.");
        }

        if let Ok(artifact) = get_artifact(db, challenge, user).await {
            _ = clear_artifact(&artifact).await;
            _ = delete_artifact(db, challenge, user).await;
        }

        let name = uuid::Uuid::new_v4().hyphenated().to_string();
        let target = CONFIG.artifact_root.join(&name);

        let path = CONFIG.challenge_root.join(&entry.path);
        let source = path.join("source");

        let flag = entry.flag.clone();

        let flag = match entry.dynamic {
            true => generate_random_flag(&flag)?,
            false => flag,
        };

        let result = conductor::build(&source, &target, true, &flag).await?;

        let artifact = ArtifactEntry {
            id: None,
            user,
            challenge,
            flag,
            path: name,
            info: result.artifacts.into(),
        };

        update_artifact(db, artifact).await?;

        Ok(())
    }
    .await;

    BUILDING.write().await.remove(&(user, challenge));

    result
}

pub async fn is_docker_running(user: i32, challenge: i32, artifact: usize) -> bool {
    DOCKER_INSTANCES
        .read()
        .await
        .contains_key(&(user, challenge, artifact))
}

pub async fn run_docker(db: &Db, user: i32, challenge: i32, artifact: usize) -> Result<()> {
    if !DOCKER_PREPARING
        .write()
        .await
        .insert((user, challenge, artifact))
    {
        bail!("docker {artifact} of challenge {challenge} for user {user} is preparing.");
    }

    let result = async {
        if is_docker_running(user, challenge, artifact).await {
            bail!("a docker instance is already running.");
        }

        let entry = get_challenge(db, challenge).await?;
        let entry = get_artifact(db, challenge, entry.dynamic.then_some(user)).await?;

        let info = entry
            .info
            .0
            .into_iter()
            .nth(artifact)
            .ok_or_else(|| anyhow!("artifact not found."))?;

        if let Artifact::Docker(docker) = &info {
            let info = conductor::run_docker(docker, &CONFIG.docker.options).await?;

            let stop_at = CONFIG
                .docker
                .expiry
                .and_then(|expiry| Instant::now().checked_add(expiry));

            let instance = DockerInstance { info, stop_at };

            DOCKER_INSTANCES
                .write()
                .await
                .insert((user, challenge, artifact), instance);

            DOCKER_PREPARING
                .write()
                .await
                .remove(&(user, challenge, artifact));

            return Ok(());
        }

        bail!("unexpected artifact type got.");
    }
    .await;

    DOCKER_PREPARING
        .write()
        .await
        .remove(&(user, challenge, artifact));

    result
}

pub async fn stop_docker(user: i32, challenge: i32, artifact: usize) -> Result<()> {
    let id = DOCKER_INSTANCES
        .read()
        .await
        .get(&(user, challenge, artifact))
        .ok_or_else(|| anyhow!("docker instance not found."))?
        .info
        .id
        .clone();

    conductor::stop_docker(&id).await?;

    let mut instances = DOCKER_INSTANCES.write().await;

    instances.remove(&(user, challenge, artifact));

    Ok(())
}

pub async fn stop_dockers(user: Option<i32>, challenge: i32) -> bool {
    let dockers: Vec<_> = DOCKER_INSTANCES
        .read()
        .await
        .iter()
        .filter(|(k, _)| user.map(|x| k.0 == x).unwrap_or(true) && k.1 == challenge)
        .map(|(k, v)| (*k, v.info.id.clone()))
        .collect();

    let mut success = true;

    for (key, id) in dockers {
        if conductor::stop_docker(&id).await.is_err() {
            success = false;
        }

        DOCKER_INSTANCES.write().await.remove(&key);
    }

    success
}

pub async fn stop_all_dockers() -> bool {
    let mut success = true;

    for instance in DOCKER_INSTANCES.read().await.values() {
        if conductor::stop_docker(&instance.info.id).await.is_err() {
            success = false;
        }
    }

    DOCKER_INSTANCES.write().await.clear();

    success
}

pub async fn stop_active_sessions(user: Option<i32>, challenge: i32) -> bool {
    stop_dockers(user, challenge).await
}

pub async fn stop_all_active_sessions() -> bool {
    stop_all_dockers().await
}

pub async fn docker_expiry(user: i32, challenge: i32, artifact: usize) -> Result<Option<Duration>> {
    Ok(DOCKER_INSTANCES
        .read()
        .await
        .get(&(user, challenge, artifact))
        .ok_or_else(|| anyhow!("docker instance not found."))?
        .stop_at
        .map(|stop_at| {
            let now = Instant::now();
            if now < stop_at {
                stop_at.duration_since(now)
            } else {
                Duration::ZERO
            }
        }))
}

fn mapped_addr(addr: &MappedAddr, port: u16) -> SocketAddr {
    let port = addr
        .ports
        .as_ref()
        .map(|mapped| {
            let ports = CONFIG
                .docker
                .options
                .ports
                .as_ref()
                .expect("'ports' should be set here.");

            port - ports.start() + mapped.start()
        })
        .unwrap_or(port);

    SocketAddr::new(addr.addr, port)
}

pub async fn get_docker_port_bindings(
    user: i32,
    challenge: i32,
    artifact: usize,
) -> Result<Vec<(String, Vec<SocketAddr>)>> {
    Ok(DOCKER_INSTANCES
        .read()
        .await
        .get(&(user, challenge, artifact))
        .ok_or_else(|| anyhow!("docker instance not found."))?
        .info
        .ports
        .clone()
        .into_iter()
        .map(|(exposed, port)| {
            (
                exposed,
                CONFIG
                    .docker
                    .mapped_addrs
                    .iter()
                    .map(|addr| mapped_addr(addr, port))
                    .collect(),
            )
        })
        .collect())
}

pub async fn open_binary(db: &Db, user: i32, challenge: i32, artifact: usize) -> Result<NamedFile> {
    let entry = get_challenge(db, challenge).await?;
    let entry = get_artifact(db, challenge, entry.dynamic.then_some(user)).await?;

    let artifact = (*entry.info)
        .get(artifact)
        .ok_or_else(|| anyhow!("artifact not found."))?;

    if let Artifact::Binary(artifact) = artifact {
        let path = CONFIG.artifact_root.join(&entry.path).join(&artifact.path);

        return Ok(NamedFile::open(path).await?);
    }

    bail!("unexpected artifact type got.");
}

pub async fn open_attachment(db: &Db, challenge: i32, attachment: usize) -> Result<NamedFile> {
    let challenge = get_challenge(db, challenge).await?;

    let attachment = (*challenge.attachments)
        .get(attachment)
        .ok_or_else(|| anyhow!("attachment not found."))?;

    let path = CONFIG
        .challenge_root
        .join(&challenge.path)
        .join("attachment")
        .join(attachment);

    Ok(NamedFile::open(path).await?)
}

pub async fn solve_challenge(db: &Db, user: i32, challenge: i32, flag: &str) -> Result<bool> {
    let entry = get_challenge(db, challenge).await?;

    let artifact = match entry.dynamic {
        true => Some(get_artifact(db, challenge, entry.dynamic.then_some(user)).await?),
        false => None,
    };

    let now = primitive_now();

    let submission = Submission {
        id: None,
        user,
        challenge,
        flag: flag.to_string(),
        time: now,
    };

    let submission = add_submission(db, submission).await?;

    let expected = artifact.as_ref().map(|x| &x.flag).unwrap_or(&entry.flag);

    if flag != expected {
        return Ok(false);
    }

    let solved = Solved {
        id: None,
        submission,
        score: None,
    };

    update_solved(db, solved).await?;

    let dynamic = entry.dynamic;

    recalculate_challenge_points_consumed(db, entry).await?;

    if let Some(artifact) = artifact {
        if CONFIG.clear_on_solved && dynamic {
            _ = clear_artifact(&artifact).await;
            _ = delete_dynamic_artifact(db, user, challenge).await;
        }
    }

    Ok(true)
}

pub fn is_publicly_available(challenge: &Challenge) -> bool {
    let categorized = CONFIG.show_uncategorized || challenge.problemset.is_some();

    categorized && challenge.public
}
