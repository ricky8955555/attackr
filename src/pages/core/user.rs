use std::collections::HashMap;

use rocket::{
    fairing::AdHoc,
    form::Form,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{context, Template};
use sha2::{Digest, Sha256};

use crate::{
    configs::user::CONFIG,
    db::{
        models::{User, UserRole},
        query::{
            challenge::list_challenges,
            problemset::list_problemsets,
            solved::list_user_solved,
            user::{add_user, get_user, get_user_by_username, update_user},
        },
        Db,
    },
    functions::{
        challenge::{calculate_user_points, is_publicly_available},
        user::{
            auth_session as functional_auth_session, destroy_session, hash_password, new_session,
            verify_password,
        },
    },
    pages::{auth_session, Error, Result, ResultFlashExt},
    utils::query::QueryResultExt,
};

use super::ResultResponseExt;

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/user");

#[derive(Debug, Clone, FromForm)]
struct Login<'r> {
    pub username: &'r str,
    pub password: &'r str,
}

#[derive(Debug, Clone, FromForm)]
struct Register<'r> {
    pub username: &'r str,
    pub password: &'r str,
    pub email: &'r str,
    pub contact: &'r str,
}

#[derive(Debug, Clone, FromForm)]
struct Edit<'r> {
    pub password: &'r str,
    pub email: &'r str,
    pub contact: &'r str,
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, db: Db) -> Result<Redirect> {
    let user = auth_session(&db, jar).await?;

    Ok(Redirect::to(uri!(ROOT, view(user.id.unwrap()))))
}

#[get("/<id>")]
async fn view(
    jar: &CookieJar<'_>,
    db: Db,
    id: i32,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template> {
    let current = functional_auth_session(&db, jar).await.ok();
    let is_self = current.as_ref().and_then(|user| user.id) == Some(id);

    let user = if is_self {
        current.as_ref().unwrap()
    } else {
        &get_user(&db, id).await.resp_expect("获取用户失败")?
    };

    if !user.enabled {
        return Err(Error::redirect(uri!(ROOT, index), "禁止查看被禁用用户主页"));
    }

    let problemsets: HashMap<_, _> = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?
        .into_iter()
        .map(|problemset| (problemset.id, problemset))
        .collect();

    let solved: HashMap<_, _> = list_user_solved(&db, id)
        .await
        .resp_expect("获取用户解题信息失败")?
        .into_iter()
        .map(|data| (data.submission.challenge, data))
        .collect();

    let progress: Vec<_> = list_challenges(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .filter(is_publicly_available)
        .map(|challenge| {
            let solved = solved.get(&challenge.id.unwrap());

            let points = solved
                .map(|data| calculate_user_points(&challenge, &data.solved))
                .unwrap_or(0.0);

            context! {
                solved,
                points,
                problemset: problemsets.get(&challenge.problemset),
                challenge,
            }
        })
        .collect();

    let email = format!(
        "{:x}",
        Sha256::new()
            .chain_update(&user.email.to_ascii_lowercase())
            .finalize()
    );

    Ok(Template::render(
        "core/user/index",
        context! {flash, user, is_self, progress, email},
    ))
}

#[get("/edit")]
async fn edit_page(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template> {
    let user = auth_session(&db, jar).await?;

    Ok(Template::render("core/user/edit", context! {flash, user}))
}

#[post("/edit", data = "<info>")]
async fn edit(jar: &CookieJar<'_>, db: Db, info: Form<Edit<'_>>) -> Result<Flash<Redirect>> {
    let user = auth_session(&db, jar).await?;

    let new_user = User {
        id: Some(user.id.unwrap()),
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
        enabled: user.enabled,
        role: user.role,
    };

    update_user(&db, new_user)
        .await
        .flash_expect(uri!(ROOT, edit_page), "修改信息失败")?;

    Ok(Flash::success(
        Redirect::to(uri!(ROOT, index)),
        "修改信息成功",
    ))
}

#[get("/login")]
fn login_page(flash: Option<FlashMessage<'_>>) -> Template {
    Template::render("core/user/login", context! {flash})
}

#[post("/login", data = "<login>")]
async fn login(jar: &CookieJar<'_>, db: Db, login: Form<Login<'_>>) -> Result<Redirect> {
    if login.username.is_empty() || login.password.is_empty() {
        return Err(Error::redirect(
            uri!(ROOT, login_page),
            "用户名和密码不能为空",
        ));
    }

    if let Ok(user) = get_user_by_username(&db, login.username.to_string()).await {
        if !user.enabled {
            return Err(Error::redirect(uri!(ROOT, login_page), "用户被禁用"));
        }
        if let Ok(valid) = verify_password(login.password, &user.password) {
            if valid {
                new_session(jar, &user).flash_expect(uri!(ROOT, login_page), "创建 Token 失败")?;
                return Ok(Redirect::to(uri!(ROOT, index)));
            }
        }
    }

    Err(Error::redirect(uri!(ROOT, login_page), "用户名或密码错误"))
}

#[get("/register")]
fn register_page(flash: Option<FlashMessage<'_>>) -> Template {
    Template::render("core/user/register", context! {flash})
}

#[post("/register", data = "<info>")]
async fn register(db: Db, info: Form<Register<'_>>) -> Result<Flash<Redirect>> {
    let user = get_user_by_username(&db, info.username.to_string())
        .await
        .some()
        .flash_expect(uri!(ROOT, register_page), "查询用户信息失败")?;

    if user.is_some() {
        return Err(Error::redirect(
            uri!(ROOT, register_page),
            &format!("用户名 {} 已被占用", info.username),
        ));
    }

    let enabled = CONFIG.no_verify;

    let user = User {
        id: None,
        username: info.username.to_string(),
        password: hash_password(info.password),
        contact: info.contact.to_string(),
        email: info.email.to_string(),
        enabled,
        role: UserRole::Challenger,
    };

    add_user(&db, user)
        .await
        .flash_expect(uri!(ROOT, register_page), "创建用户失败")?;

    Ok(if enabled {
        Flash::success(Redirect::to(uri!(ROOT, login_page)), "注册成功，请登录")
    } else {
        Flash::success(
            Redirect::to(uri!(ROOT, register_page)),
            "注册成功，请等待管理员审核",
        )
    })
}

#[get("/logout")]
fn logout(jar: &CookieJar<'_>) -> Redirect {
    destroy_session(jar);

    Redirect::to(uri!(super::root::ROOT, super::root::index))
}

pub fn stage() -> AdHoc {
    let routes = routes![
        login_page,
        login,
        register_page,
        register,
        edit_page,
        edit,
        index,
        view,
        logout
    ];

    AdHoc::on_ignite("Core Pages - User", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
