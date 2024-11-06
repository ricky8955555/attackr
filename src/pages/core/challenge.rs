use std::{cmp::Ordering, collections::HashMap, net::SocketAddr};

use itertools::Itertools;
use rocket::{
    fairing::AdHoc,
    form::Form,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{context, Template};
use serde::Serialize;
use time::OffsetDateTime;

use crate::{
    core::conductor::Artifact,
    db::{
        models::{Challenge, User},
        query::{
            artifact::get_artifact,
            challenge::{get_challenge, list_challenges},
            problemset::list_problemsets,
            solved::{
                count_challenge_effective_solved, get_solved, list_effective_solved,
                list_user_solved,
            },
        },
        Db,
    },
    functions::{
        challenge::{
            build_challenge, docker_expiry, get_docker_port_bindings, is_docker_running,
            is_publicly_available, open_attachment, open_binary, run_docker, solve_challenge,
            stop_docker,
        },
        event::cmp_period,
        user::is_admin,
    },
    pages::{auth_session, Error, Result, ResultFlashExt},
    utils::{query::QueryResultExt, responder::NamedFile},
};

use super::{check_event_availability, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/challenge");

#[derive(Debug, Clone, FromForm)]
struct Solve<'r> {
    #[field(validate = len(1..))]
    pub flag: &'r str,
}

#[derive(Debug, Clone, Serialize)]
struct DockerInfo {
    pub expiry: Option<u64>,
    pub ports: Vec<(String, Vec<SocketAddr>)>,
}

#[allow(clippy::result_large_err)]
#[inline]
fn check_challenge_availability(user: &User, challenge: &Challenge) -> Result<()> {
    if is_admin(user) {
        return Ok(());
    }

    if !is_publicly_available(challenge) {
        return Err(Error::redirect(uri!(ROOT, index), "该题目禁止访问"));
    }

    Ok(())
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let user = auth_session(&db, jar).await?;

    check_event_availability(Some(&user))?;

    let all_solved: HashMap<_, _> = list_effective_solved(&db)
        .await
        .resp_expect("获取用户解题信息失败")?
        .into_iter()
        .into_group_map_by(|data| data.submission.challenge);

    let user_solved: HashMap<_, _> = list_user_solved(&db, user.id.unwrap())
        .await
        .resp_expect("获取用户解题信息失败")?
        .into_iter()
        .map(|data| (data.submission.challenge, data))
        .collect();

    let empty_vec = Vec::new();

    let problemsets: HashMap<_, _> = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?
        .into_iter()
        .map(|problemset| (problemset.id, problemset))
        .collect();

    let info: Vec<_> = list_challenges(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .filter(|challenge| is_admin(&user) || is_publicly_available(challenge))
        .map(|challenge| {
            let challenge_id = challenge.id.unwrap();

            let solved = all_solved.get(&challenge_id).unwrap_or(&empty_vec);
            let user_solved = user_solved.get(&challenge_id);

            let points = user_solved.map(|data| data.score.points).unwrap_or(0.0);

            context! {
                problemset: problemsets.get(&challenge.problemset),
                solved,
                user_solved,
                points,
                challenge,
            }
        })
        .collect();

    Ok(Template::render(
        "core/challenge/index",
        context! {flash, info},
    ))
}

#[get("/<id>")]
async fn detail(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
    id: i32,
) -> Result<Template> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let user_id = user.id.unwrap();

    let challenge = get_challenge(&db, id).await.resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &challenge)?;

    let solved_count = count_challenge_effective_solved(&db, id)
        .await
        .resp_expect("获取解题人数失败")?;

    let solved = get_solved(&db, user_id, id)
        .await
        .some()
        .resp_expect("获取解题状态失败")?;

    let artifact = get_artifact(&db, id, challenge.dynamic.then_some(user_id))
        .await
        .some()
        .resp_expect("获取构建产物信息失败")?;

    let mut dockers = HashMap::new();

    if let Some(artifact) = &artifact {
        for (idx, artifact) in artifact.info.iter().enumerate() {
            if let Artifact::Docker(docker) = artifact {
                if is_docker_running(user_id, id, idx).await {
                    let expiry = docker_expiry(user_id, id, idx)
                        .await
                        .resp_expect("获取时效信息失败")?;

                    let ports = get_docker_port_bindings(user_id, id, idx)
                        .await
                        .resp_expect("获取端口绑定信息失败")?;

                    let info = DockerInfo {
                        expiry: expiry.map(|x| x.as_secs()),
                        ports,
                    };

                    dockers.insert(docker.id.clone(), info);
                }
            }
        }
    }

    Ok(Template::render(
        "core/challenge/detail",
        context! {flash, challenge, solved, solved_count, artifact, dockers},
    ))
}

#[get("/<id>/build")]
async fn build(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let challenge = get_challenge(&db, id).await.resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &challenge)?;

    let user_id = user.id.unwrap();

    build_challenge(&db, Some(user_id), id)
        .await
        .flash_expect(uri!(ROOT, detail(id)), "构建题目失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, detail(id))),
        "构建成功",
    ))
}

#[get("/<challenge>/artifact/binary/<artifact>")]
async fn artifact_binary(
    jar: &CookieJar<'_>,
    db: Db,
    challenge: i32,
    artifact: usize,
) -> Result<NamedFile> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let entry = get_challenge(&db, challenge)
        .await
        .resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &entry)?;

    let file = open_binary(&db, user.id.unwrap(), challenge, artifact)
        .await
        .flash_expect(uri!(ROOT, detail(challenge)), "获取构建产物失败")?;

    Ok(file)
}

#[get("/<challenge>/attachment/<attachment>")]
async fn attachment(
    jar: &CookieJar<'_>,
    db: Db,
    challenge: i32,
    attachment: usize,
) -> Result<NamedFile> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let entry = get_challenge(&db, challenge)
        .await
        .resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &entry)?;

    let file = open_attachment(&db, challenge, attachment)
        .await
        .flash_expect(uri!(ROOT, detail(challenge)), "获取附件失败")?;

    Ok(file)
}

#[get("/<challenge>/artifact/docker/<artifact>/run")]
async fn artifact_docker_run(
    jar: &CookieJar<'_>,
    db: Db,
    challenge: i32,
    artifact: usize,
) -> Result<Flash<Redirect>> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let entry = get_challenge(&db, challenge)
        .await
        .resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &entry)?;

    run_docker(&db, user.id.unwrap(), challenge, artifact)
        .await
        .flash_expect(uri!(ROOT, detail(challenge)), "启动容器失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, detail(challenge))),
        "启动容器成功",
    ))
}

#[get("/<challenge>/artifact/docker/<artifact>/stop")]
async fn artifact_docker_stop(
    jar: &CookieJar<'_>,
    db: Db,
    challenge: i32,
    artifact: usize,
) -> Result<Flash<Redirect>> {
    let user = auth_session(&db, jar).await?;
    check_event_availability(Some(&user))?;

    let entry = get_challenge(&db, challenge)
        .await
        .resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &entry)?;

    stop_docker(user.id.unwrap(), challenge, artifact)
        .await
        .flash_expect(uri!(ROOT, detail(challenge)), "停止容器失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, detail(challenge))),
        "停止容器成功",
    ))
}

#[post("/<id>/solve", data = "<solve>")]
async fn solve(
    jar: &CookieJar<'_>,
    db: Db,
    id: i32,
    solve: Form<Solve<'_>>,
) -> Result<Flash<Redirect>> {
    if cmp_period(OffsetDateTime::now_utc()) == Ordering::Greater {
        return Err(Error::redirect(
            uri!(ROOT, detail(id)),
            "禁止在比赛结束后提交 Flag",
        ));
    }

    let user = auth_session(&db, jar).await?;

    if is_admin(&user) {
        return Err(Error::redirect(
            uri!(ROOT, detail(id)),
            "禁止以管理员身份提交 Flag",
        ));
    }

    check_event_availability(Some(&user))?;

    let challenge = get_challenge(&db, id).await.resp_expect("获取题目失败")?;
    check_challenge_availability(&user, &challenge)?;

    let user_id = user.id.unwrap();

    let solved = get_solved(&db, user_id, id)
        .await
        .some()
        .flash_expect(uri!(ROOT, detail(id)), "获取解题状态失败")?;

    if solved.is_some() {
        return Err(Error::redirect(uri!(ROOT, detail(id)), "请勿重复提交 Flag"));
    }

    let solved = solve_challenge(&db, user_id, id, solve.flag)
        .await
        .flash_expect(uri!(ROOT, detail(id)), "更新解题状态失败")?;

    Ok(if solved {
        Flash::success(Redirect::to(uri!(ROOT, detail(id))), "恭喜！通过挑战！")
    } else {
        Flash::error(Redirect::to(uri!(ROOT, detail(id))), "Flag 不正确！")
    })
}

pub fn stage() -> AdHoc {
    let routes = routes![
        index,
        detail,
        build,
        solve,
        attachment,
        artifact_binary,
        artifact_docker_run,
        artifact_docker_stop
    ];

    AdHoc::on_ignite("Core Pages - Challenge", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
