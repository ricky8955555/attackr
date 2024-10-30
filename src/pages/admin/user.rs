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
        models::{User, UserRole},
        query::user::{get_user, list_users, update_user},
        Db,
    },
    functions::user::{hash_password, remove_user},
    pages::{auth_session, Error, Result, ResultFlashExt},
};
use strum::IntoEnumIterator;

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/user");

#[derive(Debug, Clone, FromForm)]
struct UserInfo<'r> {
    pub password: &'r str,
    pub contact: &'r str,
    pub email: &'r str,
    pub enabled: bool,
    pub role: UserRole,
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db, flash: Option<FlashMessage<'_>>) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let users: Vec<_> = list_users(&db)
        .await
        .resp_expect("获取用户列表失败")?
        .into_iter()
        .filter(|user| user.role < current.role)
        .collect();

    Ok(Template::render(
        "admin/user/index",
        context! {flash, users},
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

    let user = get_user(&db, id).await.resp_expect("获取用户失败")?;

    if current.role <= user.role {
        return Err(Error::redirect(
            uri!(ROOT, index),
            "禁止查看相同或更高权限组的用户信息",
        ));
    }

    let roles: Vec<_> = UserRole::iter().collect();

    Ok(Template::render(
        "admin/user/edit",
        context! {flash, user, roles},
    ))
}

#[post("/<id>", data = "<info>")]
async fn edit(
    jar: &CookieJar<'_>,
    db: Db,
    id: i32,
    info: Form<UserInfo<'_>>,
) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let user = get_user(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取用户失败")?;

    if current.role <= user.role {
        return Err(Error::redirect(
            uri!(ROOT, index),
            "禁止修改相同或更高权限组的用户信息",
        ));
    }

    if info.role >= current.role {
        return Err(Error::redirect(
            uri!(ROOT, index),
            "禁止修改用户至相同或更高权限组",
        ));
    }

    let new_user = User {
        id: Some(id),
        username: user.username,
        password: Some(info.password)
            .filter(|s| !s.is_empty())
            .map(hash_password)
            .unwrap_or(user.password)
            .to_string(),
        contact: Some(info.contact)
            .filter(|s| !s.is_empty())
            .unwrap_or(&user.contact)
            .to_string(),
        email: Some(info.email)
            .filter(|s| !s.is_empty())
            .unwrap_or(&user.email)
            .to_string(),
        enabled: info.enabled,
        role: info.role,
    };

    update_user(&db, new_user)
        .await
        .flash_expect(uri!(ROOT, index), "修改用户信息失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "修改用户信息成功",
    ))
}

#[delete("/<id>")]
async fn delete(jar: &CookieJar<'_>, db: Db, id: i32) -> Result<Flash<Redirect>> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let user = get_user(&db, id)
        .await
        .flash_expect(uri!(ROOT, index), "获取用户失败")?;

    if current.role <= user.role {
        return Err(Error::redirect(
            uri!(ROOT, index),
            "禁止删除相同或更高权限组的用户",
        ));
    }

    remove_user(&db, user.id.unwrap())
        .await
        .flash_expect(uri!(ROOT, index), "删除用户失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "删除用户成功",
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![index, edit_page, edit, delete];

    AdHoc::on_ignite("Admin Pages - Root", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
