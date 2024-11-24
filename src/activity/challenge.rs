use koto::runtime::KValue;

use crate::db::models::{Challenge, DetailedSolved, User};

use super::{as_koto_value, broadcast, ActivityKind};

pub async fn on_solved(user: &User, challenge: &Challenge, solved: &DetailedSolved, rank: i64) {
    // the value here is guaranteed to be able to interpret as koto value.
    let args = [
        as_koto_value(user).unwrap(),
        as_koto_value(challenge).unwrap(),
        as_koto_value(solved).unwrap(),
        KValue::Number(rank.into()),
    ];

    broadcast(ActivityKind::Solved, &args).await;
}
