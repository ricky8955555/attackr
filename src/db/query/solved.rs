use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{
    models::{DetailedSolved, Score, Solved, Submission, UserRole},
    schema::{scores, solved, submissions, users},
    Db,
};

fn tuple_to_struct(tuple: (Solved, Submission, Score)) -> DetailedSolved {
    DetailedSolved {
        submission: tuple.1,
        score: tuple.2,
        solved: tuple.0,
    }
}

pub async fn update_solved(db: &Db, solved: Solved) -> AnyResult<()> {
    solved.validate()?;

    db.run(move |conn| {
        diesel::replace_into(solved::table)
            .values(&solved)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_solved(db: &Db, user: i32, challenge: i32) -> QueryResult<DetailedSolved> {
    Ok(tuple_to_struct(
        db.run(move |conn| {
            solved::table
                .inner_join(submissions::table)
                .inner_join(scores::table)
                .filter(
                    submissions::user
                        .eq(user)
                        .and(submissions::challenge.eq(challenge)),
                )
                .first(conn)
        })
        .await?,
    ))
}

pub async fn list_user_solved(db: &Db, id: i32) -> QueryResult<Vec<DetailedSolved>> {
    Ok(db
        .run(move |conn| {
            solved::table
                .inner_join(submissions::table)
                .inner_join(scores::table)
                .filter(submissions::user.eq(id))
                .load(conn)
        })
        .await?
        .into_iter()
        .map(tuple_to_struct)
        .collect())
}

pub async fn count_challenge_effective_solved(db: &Db, id: i32) -> QueryResult<i64> {
    db.run(move |conn| {
        solved::table
            .inner_join(submissions::table.inner_join(users::table))
            .filter(
                users::role
                    .eq(UserRole::Challenger)
                    .and(users::enabled.eq(true))
                    .and(submissions::challenge.eq(id)),
            )
            .count()
            .get_result(conn)
    })
    .await
}

pub async fn list_challenge_effective_solved_with_submission(
    db: &Db,
    id: i32,
) -> QueryResult<Vec<(Solved, Submission)>> {
    Ok(db
        .run(move |conn| {
            solved::table
                .inner_join(submissions::table.inner_join(users::table))
                .filter(
                    users::role
                        .eq(UserRole::Challenger)
                        .and(users::enabled.eq(true))
                        .and(submissions::challenge.eq(id)),
                )
                .select((Solved::as_select(), Submission::as_select()))
                .load(conn)
        })
        .await?
        .into_iter()
        .collect())
}

pub async fn list_effective_solved(db: &Db) -> QueryResult<Vec<DetailedSolved>> {
    Ok(db
        .run(move |conn| {
            solved::table
                .inner_join(submissions::table.inner_join(users::table))
                .inner_join(scores::table)
                .filter(
                    users::role
                        .eq(UserRole::Challenger)
                        .and(users::enabled.eq(true)),
                )
                .select((
                    Solved::as_select(),
                    Submission::as_select(),
                    Score::as_select(),
                ))
                .load(conn)
        })
        .await?
        .into_iter()
        .map(tuple_to_struct)
        .collect())
}
