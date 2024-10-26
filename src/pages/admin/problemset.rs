use rocket::{
    fairing::AdHoc,
    form::Form,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{context, Template};

use crate::{
    db::{
        models::Problemset,
        query::problemset::{
            add_problemset, delete_problemset, get_problemset, list_problemsets, update_problemset,
        },
        Db,
    },
    pages::{auth_session, Result, ResultFlashExt},
};

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/problemset");

#[derive(Debug, Clone, FromForm)]
struct ProblemSetInfo<'r> {
    pub name: &'r str,
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemsets = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?;

    Ok(Template::render(
        "admin/problemset/index",
        context! {flash, problemsets},
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

    Ok(Template::render("admin/problemset/new", context! {flash}))
}

#[post("/new", data = "<info>")]
async fn new(
    jar: &CookieJar<'_>,
    db: Db,
    info: Form<ProblemSetInfo<'_>>,
) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemset = Problemset {
        id: None,
        name: info.name.to_string(),
    };

    add_problemset(&db, problemset)
        .await
        .flash_expect(uri!(ROOT, new_page), "添加题集失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "添加题集成功",
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

    let problemset = get_problemset(&db, id).await.resp_expect("获取题集失败")?;

    Ok(Template::render(
        "admin/problemset/edit",
        context! {flash, problemset},
    ))
}

#[post("/<id>", data = "<info>")]
async fn edit(
    jar: &CookieJar<'_>,
    db: Db,
    id: i32,
    info: Form<ProblemSetInfo<'_>>,
) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let problemset = get_problemset(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取题集失败")?;

    let new_problemset = Problemset {
        id: Some(id),
        name: Some(info.name)
            .filter(|s| !s.is_empty())
            .unwrap_or(&problemset.name)
            .to_string(),
    };

    update_problemset(&db, new_problemset)
        .await
        .flash_expect(uri!(ROOT, edit_page(id)), "修改题集信息失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "修改题集信息成功",
    ))
}

#[delete("/<id>")]
async fn delete(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    delete_problemset(&db, id)
        .await
        .flash_expect(uri!(ROOT, edit_page(id)), "删除题集失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "删除题集成功",
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![index, new_page, new, edit_page, edit, delete];

    AdHoc::on_ignite("Admin Pages - Problemset", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
