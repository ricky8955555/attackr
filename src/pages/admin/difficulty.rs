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
        models::Difficulty,
        query::difficulty::{
            add_difficulty, delete_difficulty, get_difficulty, list_difficulties, update_difficulty,
        },
        Db,
    },
    pages::{auth_session, Result, ResultFlashExt},
    utils::webcolor::parse_webcolor,
};

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/difficulty");

#[derive(Debug, Clone, FromForm)]
struct New<'r> {
    #[field(validate = len(1..))]
    pub name: &'r str,
    #[field(validate = try_with(|s| parse_webcolor(s)))]
    pub color: &'r str,
}

#[derive(Debug, Clone, FromForm)]
struct Edit<'r> {
    pub name: &'r str,
    #[field(validate = try_with(|s| parse_webcolor(s)))]
    pub color: &'r str,
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let difficulties = list_difficulties(&db)
        .await
        .resp_expect("获取难度列表失败")?;

    Ok(Template::render(
        "admin/difficulty/index",
        context! {flash, difficulties},
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

    Ok(Template::render("admin/difficulty/new", context! {flash}))
}

#[post("/new", data = "<info>")]
async fn new(jar: &CookieJar<'_>, db: Db, info: Form<New<'_>>) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let difficulty = Difficulty {
        id: None,
        name: info.name.to_string(),
        color: info.color.to_string(),
    };

    add_difficulty(&db, difficulty)
        .await
        .flash_expect(uri!(ROOT, new_page), "添加难度失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "添加难度成功",
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

    let difficulty = get_difficulty(&db, id).await.resp_expect("获取难度失败")?;

    Ok(Template::render(
        "admin/difficulty/edit",
        context! {flash, difficulty},
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

    let difficulty = get_difficulty(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取难度失败")?;

    let new_difficulty = Difficulty {
        id: Some(id),
        name: Some(info.name)
            .filter(|s| !s.is_empty())
            .unwrap_or(&difficulty.name)
            .to_string(),
        color: info.color.to_string(),
    };

    update_difficulty(&db, new_difficulty)
        .await
        .flash_expect(uri!(ROOT, edit_page(id)), "修改难度信息失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "修改难度信息成功",
    ))
}

#[delete("/<id>")]
async fn delete(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    delete_difficulty(&db, id)
        .await
        .flash_expect(uri!(ROOT, edit_page(id)), "删除难度失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "删除难度成功",
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![index, new_page, new, edit_page, edit, delete];

    AdHoc::on_ignite("Admin Pages - Difficulty", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
