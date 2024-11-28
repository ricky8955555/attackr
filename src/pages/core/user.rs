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
use validator::Validate;

use crate::{
    configs::user::CONFIG,
    db::{
        models::{User, UserRole},
        query::{
            challenge::list_challenges,
            difficulty::list_difficulties,
            problemset::list_problemsets,
            solved::list_user_solved,
            user::{add_user, get_user, get_user_by_username, update_user},
        },
        Db,
    },
    functions::{
        challenge::is_publicly_available,
        event::is_available as is_event_available,
        user::{
            auth_session as functional_auth_session, destroy_session, hash_password,
            invalidate_user_sessions, new_session, verify_password,
        },
    },
    pages::{auth_session, Error, Result, ResultFlashExt},
    utils::query::QueryResultExt,
};

use super::ResultResponseExt;

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/user");

#[derive(Debug, Clone, FromForm, Validate)]
struct Login<'r> {
    #[field(validate = with(|x| (3..=25).contains(&x.chars().count()) && x.chars().all(|c| c.is_ascii_alphanumeric()), "invalid username"))]
    pub username: &'r str,
    #[field(validate = with(|x| (6..).contains(&x.chars().count()), "password too short"))]
    pub password: &'r str,
}

#[derive(Debug, Clone, FromForm, Validate)]
struct Register<'r> {
    #[field(validate = with(|x| (3..=25).contains(&x.chars().count()) && x.chars().all(|c| c.is_ascii_alphanumeric()), "invalid username"))]
    pub username: &'r str,
    #[field(validate = with(|x| (6..).contains(&x.chars().count()), "password too short"))]
    pub password: &'r str,
    #[field(validate = contains('@'))]
    pub email: &'r str,
    #[field(validate = len(1..))]
    pub contact: &'r str,
    #[field(validate = with(|x| (..=60).contains(&x.chars().count()), "nickname too long"))]
    pub nickname: &'r str,
}

#[derive(Debug, Clone, FromForm, Validate)]
struct Edit<'r> {
    #[field(validate = with(|x| x.is_empty() || (3..=25).contains(&x.chars().count()) && x.chars().all(|c| c.is_ascii_alphanumeric()), "invalid username"))]
    pub username: &'r str,
    #[field(validate = with(|x| x.is_empty() || (6..).contains(&x.chars().count()), "password too short"))]
    pub password: &'r str,
    #[field(validate = with(|x| x.is_empty() || x.contains('@'), "incorrect email"))]
    pub email: &'r str,
    pub contact: &'r str,
    #[field(validate = with(|x| x.is_empty() || (..=60).contains(&x.chars().count()), "nickname too long"))]
    pub nickname: &'r str,
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

    let user = match is_self {
        true => current.as_ref().unwrap(),
        false => &get_user(&db, id).await.resp_expect("获取用户失败")?,
    };

    if !user.enabled {
        return Err(Error::redirect(uri!(ROOT, index), "禁止查看被禁用用户主页"));
    }

    // make them live longer
    let problemsets: HashMap<_, _>;
    let difficulties: HashMap<_, _>;
    let solved: HashMap<_, _>;

    let progress: Vec<_> = match is_event_available(current.as_ref()) {
        false => Vec::new(),
        true => {
            problemsets = list_problemsets(&db)
                .await
                .resp_expect("获取题集列表失败")?
                .into_iter()
                .map(|problemset| (problemset.id, problemset))
                .collect();

            difficulties = list_difficulties(&db)
                .await
                .resp_expect("获取题集列表失败")?
                .into_iter()
                .map(|difficulty| (difficulty.id, difficulty))
                .collect();

            solved = list_user_solved(&db, id)
                .await
                .resp_expect("获取用户解题信息失败")?
                .into_iter()
                .map(|data| (data.submission.challenge, data))
                .collect();

            list_challenges(&db)
                .await
                .resp_expect("获取题目列表失败")?
                .into_iter()
                .filter(is_publicly_available)
                .map(|challenge| {
                    let solved = solved.get(&challenge.id.unwrap());

                    let points = solved.map(|data| data.score.points).unwrap_or(0.0);

                    context! {
                        solved,
                        points,
                        problemset: problemsets.get(&challenge.problemset),
                        difficulty: difficulties.get(&challenge.difficulty),
                        challenge,
                    }
                })
                .collect()
        }
    };

    let email = format!(
        "{:x}",
        Sha256::new()
            .chain_update(user.email.to_ascii_lowercase())
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

    if user.username != info.username {
        let new_user = get_user_by_username(&db, info.username.to_string())
            .await
            .some()
            .flash_expect(uri!(ROOT, register_page), "查询用户信息失败")?;

        if new_user.is_some() {
            return Err(Error::redirect(
                uri!(ROOT, register_page),
                &format!("用户名 {} 已被占用", info.username),
            ));
        }
    }

    let new_password = !info.password.is_empty();

    let password = match new_password {
        true => {
            hash_password(info.password).flash_expect(uri!(ROOT, index), "生成密码 Hash 失败")?
        }
        false => user.password.clone(),
    };

    let new_user = User {
        id: Some(user.id.unwrap()),
        username: Some(info.username)
            .filter(|s| !s.is_empty())
            .unwrap_or(&user.username)
            .to_string(),
        password,
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
        nickname: Some(info.nickname)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    };

    update_user(&db, new_user)
        .await
        .flash_expect(uri!(ROOT, edit_page), "修改信息失败")?;

    if new_password {
        invalidate_user_sessions(user.id.unwrap());

        Ok(Flash::success(
            Redirect::to(uri!(ROOT, login_page)),
            "修改信息成功，修改密码后需重新登录",
        ))
    } else {
        Ok(Flash::success(
            Redirect::to(uri!(ROOT, index)),
            "修改信息成功",
        ))
    }
}

#[get("/login")]
fn login_page(flash: Option<FlashMessage<'_>>) -> Template {
    Template::render("core/user/login", context! {flash})
}

#[post("/login", data = "<login>")]
async fn login(jar: &CookieJar<'_>, db: Db, login: Form<Login<'_>>) -> Result<Redirect> {
    if let Ok(user) = get_user_by_username(&db, login.username.to_string()).await {
        if !user.enabled {
            return Err(Error::redirect(uri!(ROOT, login_page), "用户被禁用"));
        }

        let valid = verify_password(login.password, &user.password)
            .flash_expect(uri!(ROOT, login_page), "校验密码失败")?;

        if valid {
            new_session(jar, &user)
                .await
                .flash_expect(uri!(ROOT, login_page), "创建 Session 失败")?;

            return Ok(Redirect::to(uri!(ROOT, index)));
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
        password: hash_password(info.password).resp_expect("生成密码 Hash 失败")?,
        contact: info.contact.to_string(),
        email: info.email.to_string(),
        enabled,
        role: UserRole::Challenger,
        nickname: Some(info.nickname)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    };

    add_user(&db, user)
        .await
        .flash_expect(uri!(ROOT, register_page), "创建用户失败")?;

    Ok(match enabled {
        true => Flash::success(Redirect::to(uri!(ROOT, login_page)), "注册成功，请登录"),
        false => Flash::success(
            Redirect::to(uri!(ROOT, register_page)),
            "注册成功，请等待管理员审核",
        ),
    })
}

#[get("/logout")]
async fn logout(jar: &CookieJar<'_>) -> Redirect {
    _ = destroy_session(jar).await;

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
