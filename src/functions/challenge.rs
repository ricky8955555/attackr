use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    net::SocketAddr,
    path::PathBuf,
    sync::LazyLock,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Result};
use either::Either;
use futures_util::FutureExt;
use koto::prelude::*;
use moka::{future::Cache, notification::ListenerFuture};
use rocket::{fairing::AdHoc, fs::TempFile};
use tokio::{
    fs,
    sync::{Mutex, RwLock},
};

#[cfg(feature = "activity")]
use crate::{
    activity::challenge::on_solved,
    db::query::{
        problemset::get_problemset,
        solved::{count_challenge_effective_solved, get_solved},
        user::get_user,
    },
};

use crate::{
    configs::challenge::{MappedAddr, CONFIG},
    core::conductor::{self, Artifact, BuildInfo, RunDockerResult},
    db::{
        models::{Artifact as ArtifactEntry, Challenge, Score, Solved, Submission},
        query::{
            artifact::{delete_artifact, get_artifact, list_challenge_artifacts, update_artifact},
            challenge::{delete_challenge, get_challenge, list_challenges, update_challenge},
            score::add_score,
            solved::{list_challenge_effective_solved_with_submission, update_solved},
            submission::add_submission,
        },
        Db,
    },
    utils::{dynfmt, responder::NamedFile, script::KotoScript},
};

use super::event::primitive_now;

#[derive(Clone, Debug)]
struct DockerInstance {
    info: RunDockerResult,
    stop_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct DockerInstanceInfo {
    pub expiry: Option<Duration>,
    pub ports: Vec<(String, Vec<SocketAddr>)>,
}

type ArtifactIndex = (i32, i32, usize);

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

static DOCKER_INSTANCES: LazyLock<Cache<ArtifactIndex, DockerInstance>> = LazyLock::new(|| {
    let eviction_listener = move |_, v: DockerInstance, _| -> ListenerFuture {
        async move {
            if let Err(e) = conductor::stop_docker(&v.info.id).await {
                log::error!(target: "challenge", "failed to stop docker on eviction: {e:?}")
            }
        }
        .boxed()
    };

    let mut builder = Cache::builder().async_eviction_listener(eviction_listener);

    if let Some(expiry) = CONFIG.docker.expiry {
        builder = builder.time_to_live(expiry)
    }

    builder.build()
});

pub async fn uninitialize() {
    stop_all_active_sessions().await;
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Function - Challenge", |rocket| async {
        rocket.attach(AdHoc::on_shutdown(
            "Uninitialize Challenge Function",
            |_| {
                Box::pin(async move {
                    uninitialize().await;
                })
            },
        ))
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

pub async fn clear_artifact(artifact: &ArtifactEntry) {
    stop_active_sessions(artifact.user, artifact.challenge).await;

    let path = CONFIG.artifact_root.join(&artifact.path);
    conductor::clear_artifact(&path, &artifact.info).await
}

pub async fn remove_challenge(db: &Db, id: i32) -> Result<()> {
    let artifacts = list_challenge_artifacts(db, id).await?;

    for artifact in artifacts {
        clear_artifact(&artifact).await;
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

        let old_artifact = get_artifact(db, challenge, user).await;

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

        if let Ok(artifact) = old_artifact {
            clear_artifact(&artifact).await;
            _ = delete_artifact(db, artifact.id.unwrap()).await;
        }

        Ok(())
    }
    .await;

    BUILDING.write().await.remove(&(user, challenge));

    result
}

pub async fn is_challenge_building(user: Option<i32>, challenge: i32) -> bool {
    BUILDING.read().await.contains(&(user, challenge))
}

pub async fn is_docker_running(user: i32, challenge: i32, artifact: usize) -> bool {
    DOCKER_INSTANCES.contains_key(&(user, challenge, artifact))
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
                .insert((user, challenge, artifact), instance)
                .await;

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

pub async fn stop_docker(user: i32, challenge: i32, artifact: usize) {
    DOCKER_INSTANCES
        .invalidate(&(user, challenge, artifact))
        .await;
}

pub async fn stop_dockers(user: Option<i32>, challenge: i32) {
    for (idx, _) in DOCKER_INSTANCES.iter() {
        if user.map(|x| idx.0 == x).unwrap_or(true) && idx.1 == challenge {
            DOCKER_INSTANCES.invalidate(&idx).await;
        }
    }
}

pub async fn stop_all_dockers() {
    for (idx, _) in DOCKER_INSTANCES.iter() {
        DOCKER_INSTANCES.invalidate(&idx).await;
    }
}

pub async fn stop_active_sessions(user: Option<i32>, challenge: i32) {
    stop_dockers(user, challenge).await;
}

pub async fn stop_all_active_sessions() {
    stop_all_dockers().await;
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

pub async fn get_docker_instance_info(
    user: i32,
    challenge: i32,
    artifact: usize,
) -> Result<DockerInstanceInfo> {
    let instance = DOCKER_INSTANCES
        .get(&(user, challenge, artifact))
        .await
        .ok_or_else(|| anyhow!("docker instance not found."))?;

    let expiry = instance.stop_at.map(|stop_at| {
        let now = Instant::now();
        if now < stop_at {
            stop_at.duration_since(now)
        } else {
            Duration::ZERO
        }
    });

    let ports: Vec<_> = instance
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
        .collect();

    Ok(DockerInstanceInfo { expiry, ports })
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

    recalculate_challenge_points_consumed(db, entry.clone()).await?;

    if let Some(artifact) = artifact {
        if CONFIG.clear_on_solved && entry.dynamic {
            clear_artifact(&artifact).await;
            _ = delete_artifact(db, artifact.id.unwrap()).await;
        }
    }

    #[cfg(feature = "activity")]
    {
        let rank = count_challenge_effective_solved(db, challenge).await?;
        let solved = get_solved(db, user, challenge).await?;
        let user = get_user(db, user).await?;
        let problemset = match entry.problemset {
            Some(id) => Some(get_problemset(db, id).await?),
            None => None,
        };

        on_solved(&user, &entry, problemset.as_ref(), &solved, rank).await;
    }

    Ok(true)
}

pub fn is_publicly_available(challenge: &Challenge) -> bool {
    let categorized = CONFIG.show_uncategorized || challenge.problemset.is_some();

    categorized && challenge.public
}
