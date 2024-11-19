use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{models::Artifact, schema::artifacts, Db};

pub async fn update_artifact(db: &Db, artifact: Artifact) -> AnyResult<()> {
    artifact.validate()?;

    db.run(move |conn| {
        diesel::replace_into(artifacts::table)
            .values(&artifact)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_static_artifact(db: &Db, challenge: i32) -> QueryResult<Artifact> {
    db.run(move |conn| {
        artifacts::table
            .filter(
                artifacts::user
                    .is_null()
                    .and(artifacts::challenge.eq(challenge)),
            )
            .first(conn)
    })
    .await
}

pub async fn get_dynamic_artifact(db: &Db, challenge: i32, user: i32) -> QueryResult<Artifact> {
    db.run(move |conn| {
        artifacts::table
            .filter(
                artifacts::user
                    .eq(user)
                    .and(artifacts::challenge.eq(challenge)),
            )
            .first(conn)
    })
    .await
}

pub async fn get_artifact(db: &Db, challenge: i32, user: Option<i32>) -> QueryResult<Artifact> {
    if let Some(user) = user {
        get_dynamic_artifact(db, challenge, user).await
    } else {
        get_static_artifact(db, challenge).await
    }
}

pub async fn get_artifact_by_id(db: &Db, id: i32) -> QueryResult<Artifact> {
    db.run(move |conn| artifacts::table.filter(artifacts::id.eq(id)).first(conn))
        .await
}

pub async fn delete_artifact(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::delete(artifacts::table)
            .filter(artifacts::id.eq(id))
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn list_artifacts(db: &Db) -> QueryResult<Vec<Artifact>> {
    db.run(move |conn| artifacts::table.load(conn)).await
}

pub async fn list_user_artifacts(db: &Db, id: i32) -> QueryResult<Vec<Artifact>> {
    db.run(move |conn| artifacts::table.filter(artifacts::user.eq(id)).load(conn))
        .await
}

pub async fn list_challenge_artifacts(db: &Db, id: i32) -> QueryResult<Vec<Artifact>> {
    db.run(move |conn| {
        artifacts::table
            .filter(artifacts::challenge.eq(id))
            .load(conn)
    })
    .await
}
