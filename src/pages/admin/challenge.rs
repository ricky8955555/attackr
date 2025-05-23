use std::collections::HashMap;

use rocket::{
    fairing::AdHoc,
    form::Form,
    fs::TempFile,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{context, Template};

use crate::{
    db::{
        models::Challenge,
        query::{
            challenge::{
                add_challenge, delete_challenge, get_challenge, list_challenges,
                list_private_challenges, publish_challenge, update_challenge,
            },
            difficulty::list_difficulties,
            problemset::list_problemsets,
        },
        Db,
    },
    functions::challenge::{
        build_challenge, load_build_info, recalculate_challenge_points, recalculate_points,
        remove_challenge, save_files,
    },
    pages::{auth_session, Error, Result, ResultFlashExt},
};

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/challenge");

#[derive(Debug, FromForm)]
struct Edit<'r> {
    pub name: &'r str,
    pub description: &'r str,
    pub problemset: Option<i32>,
    #[field(validate = with(|x| x.map(|v| v >= 1.0).unwrap_or(true), "points too low."))]
    pub points: Option<f64>,
    pub public: bool,
    pub difficulty: Option<i32>,
}

#[derive(Debug, FromForm)]
struct New<'r> {
    #[field(validate = len(1..))]
    pub name: &'r str,
    pub description: &'r str,
    #[field(validate = with(|x| *x >= 1.0, "points too low."))]
    pub points: f64,
    pub problemset: Option<i32>,
    pub source: Option<TempFile<'r>>,
    pub attachments: Option<Vec<TempFile<'r>>>,
    pub dynamic: bool,
    pub flag: &'r str,
    pub public: bool,
    pub difficulty: Option<i32>,
}

#[derive(Debug, FromForm)]
struct Publish {
    pub challenges: Vec<i32>,
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemsets: HashMap<_, _> = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?
        .into_iter()
        .map(|problemset| (problemset.id, problemset))
        .collect();

    let difficulties: HashMap<_, _> = list_difficulties(&db)
        .await
        .resp_expect("获取难度列表失败")?
        .into_iter()
        .map(|difficulty| (difficulty.id, difficulty))
        .collect();

    let challenges: Vec<_> = list_challenges(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .map(|challenge| {
            context! {
                problemset: problemsets.get(&challenge.problemset),
                difficulty: difficulties.get(&challenge.difficulty),
                challenge,
            }
        })
        .collect();

    Ok(Template::render(
        "admin/challenge/index",
        context! {flash, challenges},
    ))
}

#[get("/publish")]
async fn publish_page(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemsets: HashMap<_, _> = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?
        .into_iter()
        .map(|problemset| (problemset.id, problemset))
        .collect();

    let difficulties: HashMap<_, _> = list_difficulties(&db)
        .await
        .resp_expect("获取难度列表失败")?
        .into_iter()
        .map(|difficulty| (difficulty.id, difficulty))
        .collect();

    let challenges: Vec<_> = list_private_challenges(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .map(|challenge| {
            context! {
                problemset: problemsets.get(&challenge.problemset),
                difficulty: difficulties.get(&challenge.difficulty),
                challenge,
            }
        })
        .collect();

    Ok(Template::render(
        "admin/challenge/publish",
        context! {flash, challenges},
    ))
}

#[post("/publish", data = "<info>")]
async fn publish(jar: &CookieJar<'_>, db: Db, info: Form<Publish>) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    for id in &info.challenges {
        publish_challenge(&db, *id)
            .await
            .flash_expect(uri!(ROOT, publish_page), &format!("公开 ID {id} 题目失败"))?;
    }

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "公开题目成功",
    ))
}

#[get("/new")]
async fn new_page(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemsets = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?;

    let difficulties = list_difficulties(&db)
        .await
        .resp_expect("获取难度列表失败")?;

    Ok(Template::render(
        "admin/challenge/new",
        context! {flash, problemsets, difficulties},
    ))
}

#[post("/new", data = "<info>")]
async fn new(jar: &CookieJar<'_>, db: Db, mut info: Form<New<'_>>) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let has_source = info.source.is_some();
    let source = info.source.take();

    if info.dynamic && !has_source {
        return Err(Error::redirect(uri!(ROOT, new_page), "未上传源代码"));
    }

    if info.flag.is_empty() {
        if info.dynamic {
            info.flag = "flag{{{}}}";
        } else {
            return Err(Error::redirect(uri!(ROOT, new_page), "Flag 不能为空"));
        }
    }

    let attachments = info.attachments.take().unwrap_or_default();

    let (path, attachments) = save_files(source, attachments)
        .await
        .flash_expect(uri!(ROOT, new_page), "保存文件失败")?;

    let challenge = Challenge {
        id: None,
        name: info.name.to_string(),
        description: info.description.to_string(),
        path,
        attachments: attachments.into(),
        problemset: info.problemset,
        dynamic: info.dynamic,
        flag: info.flag.to_string(),
        initial: info.points,
        points: info.points,
        public: info.public,
        difficulty: info.difficulty,
    };

    let challenge = add_challenge(&db, challenge)
        .await
        .flash_expect(uri!(ROOT, new_page), "添加题目失败")?;

    if !info.dynamic && has_source {
        let result = build_challenge(&db, None, challenge).await;

        if result.is_err() {
            _ = delete_challenge(&db, challenge).await;
        }

        result.flash_expect(uri!(ROOT, new_page), "构建题目失败")?;
    }

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "添加题目成功",
    ))
}

#[get("/<id>")]
async fn edit_page(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
    id: i32,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let challenge = get_challenge(&db, id).await.resp_expect("获取题目失败")?;

    let problemsets = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?;

    let difficulties = list_difficulties(&db)
        .await
        .resp_expect("获取难度列表失败")?;

    Ok(Template::render(
        "admin/challenge/edit",
        context! {flash, challenge, problemsets, difficulties},
    ))
}

#[post("/<id>", data = "<info>")]
async fn edit(
    jar: &CookieJar<'_>,
    db: Db,
    id: i32,
    info: Form<Edit<'_>>,
) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let challenge = get_challenge(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取题目失败")?;

    let points = info.points.unwrap_or(challenge.initial);
    let recalculate = points != challenge.initial;

    let new_challenge = Challenge {
        id: Some(id),
        name: Some(info.name)
            .filter(|s| !s.is_empty())
            .unwrap_or(&challenge.name)
            .to_string(),
        description: info.description.to_string(),
        path: challenge.path,
        problemset: info.problemset,
        attachments: challenge.attachments,
        dynamic: challenge.dynamic,
        flag: challenge.flag,
        initial: points,
        points: challenge.points,
        public: info.public,
        difficulty: info.difficulty,
    };

    update_challenge(&db, new_challenge)
        .await
        .flash_expect(uri!(ROOT, edit_page(id)), "修改题目信息失败")?;

    if recalculate {
        recalculate_challenge_points(&db, id)
            .await
            .flash_expect(uri!(ROOT, edit_page(id)), "重新计算题目分数失败")?;
    }

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "修改题目信息成功",
    ))
}

#[delete("/<id>")]
async fn delete(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    remove_challenge(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "删除题目失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "删除题目成功",
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

    let challenge = get_challenge(&db, id).await.resp_expect("获取题目失败")?;
    let build = load_build_info(&db, id).await.ok();

    Ok(Template::render(
        "admin/challenge/detail",
        context! {flash, challenge, build},
    ))
}

#[get("/recalculate")]
async fn recalculate(jar: &CookieJar<'_>, db: Db) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    recalculate_points(&db)
        .await
        .flash_expect(uri!(ROOT, index), "重新计算分数失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "重新计算分数成功",
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![
        index,
        publish_page,
        publish,
        new_page,
        new,
        edit_page,
        edit,
        delete,
        detail,
        recalculate
    ];

    AdHoc::on_ignite("Admin Pages - Challenge", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
