use diesel::prelude::*;

use anyhow::Result as AnyResult;
use diesel::QueryResult;
use validator::Validate;

use crate::db::{
    models::{User, UserRole},
    schema::users,
    Db,
};

pub async fn add_user(db: &Db, user: User) -> AnyResult<i32> {
    user.validate()?;

    Ok(db
        .run(move |conn| {
            diesel::insert_into(users::table)
                .values(&user)
                .returning(users::id)
                .get_result(conn)
        })
        .await
        .map(|id: Option<i32>| id.expect("returning guarantees id present"))?)
}

pub async fn update_user(db: &Db, user: User) -> AnyResult<()> {
    user.validate()?;

    db.run(move |conn| {
        diesel::update(users::table.filter(users::id.eq(user.id)))
            .set(&user)
            .execute(conn)
    })
    .await?;

    Ok(())
}

pub async fn get_user(db: &Db, id: i32) -> QueryResult<User> {
    db.run(move |conn| users::table.filter(users::id.eq(id)).first(conn))
        .await
}

pub async fn list_users(db: &Db) -> QueryResult<Vec<User>> {
    db.run(move |conn| users::table.load(conn)).await
}

pub async fn list_active_challengers(db: &Db) -> QueryResult<Vec<User>> {
    db.run(move |conn| {
        users::table
            .filter(
                users::role
                    .eq(UserRole::Challenger)
                    .and(users::enabled.eq(true)),
            )
            .load(conn)
    })
    .await
}

pub async fn get_user_by_username(db: &Db, username: String) -> QueryResult<User> {
    db.run(move |conn| {
        users::table
            .filter(users::username.eq(username))
            .first(conn)
    })
    .await
}

pub async fn delete_user(db: &Db, id: i32) -> QueryResult<()> {
    db.run(move |conn| {
        diesel::delete(users::table)
            .filter(users::id.eq(id))
            .execute(conn)
    })
    .await?;

    Ok(())
}
