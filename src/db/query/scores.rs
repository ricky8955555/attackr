use diesel::prelude::*;

use anyhow::Result as AnyResult;
use validator::Validate;

use crate::db::{models::Score, schema::scores, Db};

pub async fn add_score(db: &Db, score: Score) -> AnyResult<i32> {
    score.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(scores::table)
                .values(&score)
                .returning(scores::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn list_scores(db: &Db) -> QueryResult<Vec<Score>> {
    db.run(move |conn| scores::table.load(conn)).await
}
