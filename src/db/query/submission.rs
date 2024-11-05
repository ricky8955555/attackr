use diesel::prelude::*;

use anyhow::Result as AnyResult;
use validator::Validate;

use crate::db::{models::Submission, schema::submissions, Db};

pub async fn add_submission(db: &Db, submission: Submission) -> AnyResult<i32> {
    submission.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(submissions::table)
                .values(&submission)
                .returning(submissions::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn list_submissions(db: &Db) -> QueryResult<Vec<Submission>> {
    db.run(move |conn| submissions::table.load(conn)).await
}

pub async fn list_user_submissions(db: &Db, id: i32) -> QueryResult<Vec<Submission>> {
    db.run(move |conn| {
        submissions::table
            .filter(submissions::user.eq(id))
            .load(conn)
    })
    .await
}

pub async fn list_challenge_submissions(db: &Db, id: i32) -> QueryResult<Vec<Submission>> {
    db.run(move |conn| {
        submissions::table
            .filter(submissions::challenge.eq(id))
            .load(conn)
    })
    .await
}

pub async fn list_user_challenge_submissions(
    db: &Db,
    user: i32,
    challenge: i32,
) -> QueryResult<Vec<Submission>> {
    db.run(move |conn| {
        submissions::table
            .filter(
                submissions::user
                    .eq(user)
                    .and(submissions::challenge.eq(challenge)),
            )
            .load(conn)
    })
    .await
}
