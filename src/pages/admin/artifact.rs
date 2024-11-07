use std::collections::HashMap;

use rocket::{
    fairing::AdHoc,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{context, Template};

use crate::{
    db::{
        query::{
            artifact::{delete_artifact_by_id, get_artifact_by_id, list_artifacts},
            challenge::{get_challenge, list_challenges},
            user::{get_user, list_users},
        },
        Db,
    },
    functions::challenge::{build_challenge, clear_artifact},
    pages::{auth_session, OptionFlashExt, Result, ResultFlashExt},
};

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/artifact");

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let users: HashMap<_, _> = list_users(&db)
        .await
        .resp_expect("获取用户列表失败")?
        .into_iter()
        .map(|user| (user.id.unwrap(), user))
        .collect();

    let challenges: HashMap<_, _> = list_challenges(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .map(|challenge| (challenge.id.unwrap(), challenge))
        .collect();

    let artifacts: Vec<_> = list_artifacts(&db)
        .await
        .resp_expect("获取产物列表失败")?
        .into_iter()
        .map(|artifact| {
            context! {
                user: artifact.user.map(|user| users.get(&user)),
                challenge: challenges.get(&artifact.challenge).expect("foreign key"),
                artifact,
            }
        })
        .collect();

    Ok(Template::render(
        "admin/artifact/index",
        context! { flash, artifacts },
    ))
}

#[get("/<id>/detail")]
async fn detail(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
    id: i32,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let artifact = get_artifact_by_id(&db, id)
        .await
        .resp_expect("获取产物失败")?;

    let user = match artifact.user {
        Some(user) => Some(get_user(&db, user).await.resp_expect("获取用户失败")?),
        None => None,
    };

    let challenge = get_challenge(&db, artifact.challenge)
        .await
        .resp_expect("获取题目失败")?;

    Ok(Template::render(
        "admin/artifact/detail",
        context! { flash, artifact, user, challenge },
    ))
}

#[get("/<id>/rebuild")]
async fn rebuild(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let artifact = get_artifact_by_id(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取产物失败")?;

    build_challenge(&db, artifact.user, artifact.challenge)
        .await
        .flash_expect(uri!(ROOT, index), "构建产物失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "重新构建产物成功",
    ))
}

#[get("/<id>/delete")]
async fn delete(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let artifact = get_artifact_by_id(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取产物失败")?;

    _ = artifact
        .user
        .flash_expect(uri!(ROOT, detail(id)), "禁止删除静态产物")?;

    let cleared = clear_artifact(&artifact).await;

    delete_artifact_by_id(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "删除产物失败")?;

    if cleared {
        Ok(Flash::success(
            Redirect::to(uri!(ROOT, index)),
            "清理并删除产物成功",
        ))
    } else {
        Ok(Flash::warning(
            Redirect::to(uri!(ROOT, index)),
            "删除产物成功，但清理产物时发生错误",
        ))
    }
}

pub fn stage() -> AdHoc {
    let routes = routes![index, detail, rebuild, delete];

    AdHoc::on_ignite("Admin Pages - Artifact", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
