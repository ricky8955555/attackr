use std::collections::HashMap;

use rocket::{
    fairing::AdHoc,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
};
use rocket_dyn_templates::{context, Template};

use crate::{
    db::{
        query::{
            challenge::list_challenges,
            submission::{
                list_challenge_submissions, list_submissions, list_user_challenge_submissions,
                list_user_submissions,
            },
            user::list_users,
        },
        Db,
    },
    pages::{auth_session, Result},
};

use super::{check_permission, OptionResponseExt, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin/submission");

#[get("/?<user>&<challenge>")]
async fn index(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
    user: Option<i32>,
    challenge: Option<i32>,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    let user_list = list_users(&db).await.resp_expect("获取用户列表失败")?;
    let users: HashMap<_, _> = user_list
        .iter()
        .map(|user| (user.id.unwrap(), user))
        .collect();

    let user_entry = match &user {
        Some(id) => Some(users.get(id).resp_expect("用户不存在")?),
        None => None,
    };

    let challenge_list = list_challenges(&db).await.resp_expect("获取题目列表失败")?;
    let challenges: HashMap<_, _> = challenge_list
        .iter()
        .map(|challenge| (challenge.id.unwrap(), challenge))
        .collect();

    let challenge_entry = match &challenge {
        Some(id) => Some(challenges.get(id).resp_expect("题目不存在")?),
        None => None,
    };

    let submissions: Vec<_> = match (user, challenge) {
        (None, None) => list_submissions(&db).await,
        (Some(user), None) => list_user_submissions(&db, user).await,
        (None, Some(challenge)) => list_challenge_submissions(&db, challenge).await,
        (Some(user), Some(challenge)) => {
            list_user_challenge_submissions(&db, user, challenge).await
        }
    }
    .resp_expect("获取题集列表失败")?
    .into_iter()
    .filter(|submission| {
        user.map(|id| submission.user == id).unwrap_or(true)
            && challenge
                .map(|id| submission.challenge == id)
                .unwrap_or(true)
    })
    .map(|submission| {
        context! {
            user: users.get(&submission.user),
            challenge: challenges.get(&submission.challenge),
            submission
        }
    })
    .collect();

    Ok(Template::render(
        "admin/submission/index",
        context! {
            flash,
            submissions,
            user: user_entry,
            challenge: challenge_entry,
            users: &user_list,
            challenges: &challenge_list,
        },
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![index];

    AdHoc::on_ignite("Admin Pages - Submission", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
