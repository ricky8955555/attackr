use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{
    models::{Solved, SolvedWithSubmission, Submission, UserRole},
    schema::{solved, submissions, users},
    Db,
};

fn tuple_to_struct(tuple: (Solved, Submission)) -> SolvedWithSubmission {
    SolvedWithSubmission {
        submission: tuple.1,
        solved: tuple.0,
    }
}

pub async fn add_solved(db: &Db, solved: Solved) -> AnyResult<i32> {
    solved.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(solved::table)
                .values(&solved)
                .returning(solved::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn update_all_solved(db: &Db, solved: Vec<Solved>) -> AnyResult<()> {
    solved.validate()?;

    db.run(move |conn| {
        diesel::replace_into(solved::table)
            .values(&solved)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_solved(db: &Db, user: i32, challenge: i32) -> QueryResult<SolvedWithSubmission> {
    Ok(tuple_to_struct(
        db.run(move |conn| {
            solved::table
                .inner_join(submissions::table)
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

pub async fn list_user_solved(db: &Db, id: i32) -> QueryResult<Vec<SolvedWithSubmission>> {
    Ok(db
        .run(move |conn| {
            solved::table
                .inner_join(submissions::table)
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

pub async fn list_challenge_ordered_effective_solved(
    db: &Db,
    id: i32,
) -> QueryResult<Vec<SolvedWithSubmission>> {
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
                .order(submissions::time.asc())
                .load(conn)
        })
        .await?
        .into_iter()
        .map(tuple_to_struct)
        .collect())
}

pub async fn list_ordered_effective_solved(db: &Db) -> QueryResult<Vec<SolvedWithSubmission>> {
    Ok(db
        .run(move |conn| {
            solved::table
                .inner_join(submissions::table.inner_join(users::table))
                .filter(
                    users::role
                        .eq(UserRole::Challenger)
                        .and(users::enabled.eq(true)),
                )
                .select((Solved::as_select(), Submission::as_select()))
                .order(submissions::time.asc())
                .load(conn)
        })
        .await?
        .into_iter()
        .map(tuple_to_struct)
        .collect())
}
