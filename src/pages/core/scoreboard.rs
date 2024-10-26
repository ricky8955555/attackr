use std::{cmp::Ordering, collections::HashMap};

use rocket::{
    fairing::AdHoc,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
};
use rocket_dyn_templates::{context, Template};
use time::PrimitiveDateTime;

use crate::{
    db::{
        query::{
            challenge::{list_challenges, list_problemset_challenges},
            problemset::list_problemsets,
            solved::list_ordered_effective_solved,
            user::list_active_challengers,
        },
        Db,
    },
    functions::{
        challenge::{calculate_user_points, is_publicly_available},
        event::primitive_now,
        user::auth_session,
    },
    pages::{Error, Result},
};

use super::{check_event_availability, ResultResponseExt};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/scoreboard");

#[get("/?<problemset>")]
async fn index(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
    problemset: Option<i32>,
) -> Result<Template> {
    let user = auth_session(&db, jar).await.ok();

    check_event_availability(user.as_ref())?;

    let problemsets = list_problemsets(&db)
        .await
        .resp_expect("获取题集列表失败")?;

    if let Some(id) = problemset {
        if !problemsets
            .iter()
            .any(|problemset| problemset.id.unwrap() == id)
        {
            return Err(Error::redirect(
                uri!(ROOT, index(None::<i32>)),
                &format!("未找到 ID {id} 题集"),
            ));
        }
    }

    let challenges: Vec<_> = match problemset {
        Some(id) => list_problemset_challenges(&db, id).await,
        None => list_challenges(&db).await,
    }
    .resp_expect("获取题目列表失败")?
    .into_iter()
    .filter(is_publicly_available)
    .collect();

    let solved: HashMap<_, _> = list_ordered_effective_solved(&db)
        .await
        .resp_expect("获取解题信息失败")?
        .into_iter()
        .map(|data| ((data.submission.user, data.submission.challenge), data))
        .collect();

    let zero_point = (PrimitiveDateTime::MIN, 0.0);

    let mut progresses: Vec<_> = list_active_challengers(&db)
        .await
        .resp_expect("获取题目列表失败")?
        .into_iter()
        .map(|user| {
            let solved: Vec<_> = challenges
                .iter()
                .map(|challenge| {
                    let solved = solved.get(&(user.id.unwrap(), challenge.id.unwrap()));
                    let points = solved
                        .map(|data| calculate_user_points(challenge, &data.solved))
                        .unwrap_or(0.0);

                    context! {solved, points}
                })
                .collect();

            let dataset: Vec<_> = solved
                .iter()
                .filter_map(|x| x.solved.map(|data| (&data.submission, x.points)))
                .collect();

            // unnecessary here because query will do it for us.
            // dataset.sort_unstable_by_key(|x| x.0.time);

            let mut dataset = dataset.into_iter().fold(vec![zero_point], |mut v, x| {
                v.push((x.0.time, v.last().unwrap().1 + x.1));
                v
            });

            dataset.remove(0);

            context! { dataset, solved, user }
        })
        .collect();

    progresses.sort_unstable_by(|a, b| {
        let a = a.dataset.last().unwrap_or(&zero_point);
        let b = b.dataset.last().unwrap_or(&zero_point);

        // if partial cmp failed, total cmp will used
        match a.1.partial_cmp(&b.1).unwrap_or_else(|| a.1.total_cmp(&b.1)) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal => a.0.cmp(&b.0),
        }
    });

    let now = primitive_now();

    Ok(Template::render(
        "core/scoreboard/index",
        context! {flash, challenges, progresses, problemsets, current: problemset, now},
    ))
}

pub fn stage() -> AdHoc {
    let routes = routes![index];

    AdHoc::on_ignite("Core Pages - Scoreboard", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
