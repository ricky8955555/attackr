use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{models::Problemset, schema::problemsets, Db};

pub async fn add_problemset(db: &Db, problemset: Problemset) -> AnyResult<i32> {
    problemset.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(problemsets::table)
                .values(&problemset)
                .returning(problemsets::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn update_problemset(db: &Db, problemset: Problemset) -> AnyResult<()> {
    problemset.validate()?;

    db.run(move |conn| {
        diesel::update(problemsets::table.filter(problemsets::id.eq(problemset.id)))
            .set(&problemset)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_problemset(db: &Db, id: i32) -> QueryResult<Problemset> {
    db.run(move |conn| {
        problemsets::table
            .filter(problemsets::id.eq(id))
            .first(conn)
    })
    .await
}

pub async fn list_problemsets(db: &Db) -> QueryResult<Vec<Problemset>> {
    db.run(move |conn| problemsets::table.load(conn)).await
}

pub async fn delete_problemset(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::delete(problemsets::table)
            .filter(problemsets::id.eq(id))
            .execute(conn)
    })
    .await?;

    Ok(())
}
