use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{models::Challenge, schema::challenges, Db};

pub async fn add_challenge(db: &Db, challenge: Challenge) -> AnyResult<i32> {
    challenge.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(challenges::table)
                .values(&challenge)
                .returning(challenges::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn update_challenge(db: &Db, challenge: Challenge) -> AnyResult<()> {
    challenge.validate()?;

    db.run(move |conn| {
        diesel::update(challenges::table.filter(challenges::id.eq(challenge.id)))
            .set(&challenge)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn publish_challenge(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::update(challenges::table.filter(challenges::id.eq(id)))
            .set(challenges::public.eq(true))
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_challenge(db: &Db, id: i32) -> QueryResult<Challenge> {
    db.run(move |conn| challenges::table.filter(challenges::id.eq(id)).first(conn))
        .await
}

pub async fn list_problemset_challenges(db: &Db, id: i32) -> QueryResult<Vec<Challenge>> {
    db.run(move |conn| {
        challenges::table
            .filter(challenges::problemset.eq(id))
            .load(conn)
    })
    .await
}

pub async fn list_challenges(db: &Db) -> QueryResult<Vec<Challenge>> {
    db.run(move |conn| challenges::table.load(conn)).await
}

pub async fn list_private_challenges(db: &Db) -> QueryResult<Vec<Challenge>> {
    db.run(move |conn| {
        challenges::table
            .filter(challenges::public.eq(false))
            .load(conn)
    })
    .await
}

pub async fn delete_challenge(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::delete(challenges::table)
            .filter(challenges::id.eq(id))
            .execute(conn)
    })
    .await?;

    Ok(())
}
