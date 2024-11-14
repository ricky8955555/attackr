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
        query::user::{get_user, get_user_by_username, list_users, update_user},
        Db,
    },
    functions::user::{generate_random, hash_password, remove_user},
    pages::{auth_session, Error, Result, ResultFlashExt},
    utils::query::QueryResultExt,
};
use strum::IntoEnumIterator;

use super::{check_permission, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/user");

#[derive(Debug, Clone, FromForm)]
struct UserInfo<'r> {
    #[field(validate = with(|x| x.is_empty() || (3..=25).contains(&x.len()) && x.chars().all(|c| c.is_ascii_alphanumeric()), "invalid username"))]
    pub username: &'r str,
    #[field(validate = with(|x| x.is_empty() || (6..).contains(&x.len()), "password too short"))]
    pub password: &'r str,
    #[field(validate = with(|x| x.is_empty() || x.contains('@'), "incorrect email"))]
    pub email: &'r str,
    pub contact: &'r str,
    #[field(validate = with(|x| x.is_empty() || (..=60).contains(&x.len()), "nickname too long"))]
    pub nickname: &'r str,
    pub enabled: bool,
    pub role: UserRole,
    pub random: bool,
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

    if user.username != info.username {
        let new_user = get_user_by_username(&db, info.username.to_string())
            .await
            .some()
            .flash_expect(uri!(ROOT, edit_page(id)), "查询用户信息失败")?;

        if new_user.is_some() {
            return Err(Error::redirect(
                uri!(ROOT, edit_page(id)),
                &format!("用户名 {} 已被占用", info.username),
            ));
        }
    }

    let new_user = User {
        id: Some(id),
        username: Some(info.username)
            .filter(|s| !s.is_empty())
            .unwrap_or(&user.username)
            .to_string(),
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
        nickname: Some(info.nickname)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        random: info.random
            .then(generate_random)
            .unwrap_or_else(|| user.random.to_string()),
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
