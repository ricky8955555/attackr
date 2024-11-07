use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{models::Difficulty, schema::difficulties, Db};

pub async fn add_difficulty(db: &Db, difficulty: Difficulty) -> AnyResult<i32> {
    difficulty.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(difficulties::table)
                .values(&difficulty)
                .returning(difficulties::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn update_difficulty(db: &Db, difficulty: Difficulty) -> AnyResult<()> {
    difficulty.validate()?;

    db.run(move |conn| {
        diesel::update(difficulties::table.filter(difficulties::id.eq(difficulty.id)))
            .set(&difficulty)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_difficulty(db: &Db, id: i32) -> QueryResult<Difficulty> {
    db.run(move |conn| {
        difficulties::table
            .filter(difficulties::id.eq(id))
            .first(conn)
    })
    .await
}

pub async fn list_difficulties(db: &Db) -> QueryResult<Vec<Difficulty>> {
    db.run(move |conn| difficulties::table.load(conn)).await
}

pub async fn delete_difficulty(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::delete(difficulties::table)
            .filter(difficulties::id.eq(id))
            .execute(conn)
    })
    .await?;

    Ok(())
}
