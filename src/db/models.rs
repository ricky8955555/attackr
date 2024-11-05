use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use time::PrimitiveDateTime;
use validator::{Validate, ValidationError};

use crate::core::conductor::Artifact as ArtifactInfo;

use super::{schema::*, types::Json};

#[repr(u8)]
#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    EnumIter,
    FromFormField,
    DbEnum,
    Serialize,
    Deserialize,
)]
pub enum UserRole {
    Challenger,
    Administrator,
    Superuser,
}

impl Default for UserRole {
    fn default() -> Self {
        Self::Challenger
    }
}

fn validate_username(username: &str) -> Result<(), ValidationError> {
    if !username.is_ascii() || username.contains(' ') {
        return Err(ValidationError::new(
            "username should not contains non-ascii characters or spaces.",
        ));
    }

    Ok(())
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = users)]
#[diesel(treat_none_as_null = true)]
pub struct User {
    pub id: Option<i32>,
    #[validate(length(min = 1, max = 25), custom(function = "validate_username"))]
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    #[validate(length(min = 1))]
    pub contact: String,
    #[validate(email)]
    pub email: String,
    pub enabled: bool,
    pub role: UserRole,
    #[validate(length(min = 1, max = 60))]
    pub nickname: Option<String>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(table_name = problemsets)]
pub struct Problemset {
    pub id: Option<i32>,
    #[validate(length(min = 1))]
    pub name: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Associations,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(belongs_to(Problemset, foreign_key = problemset))]
#[diesel(table_name = challenges)]
#[diesel(treat_none_as_null = true)]
pub struct Challenge {
    pub id: Option<i32>,
    #[validate(length(min = 1))]
    pub name: String,
    pub description: String,
    #[validate(length(min = 1))]
    pub path: String,
    #[validate(range(min = 1.0))]
    pub initial: f64,
    #[validate(range(min = 1.0))]
    pub points: f64,
    pub problemset: Option<i32>,
    pub attachments: Json<Vec<String>>,
    pub flag: String,
    pub dynamic: bool,
    pub public: bool,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Associations,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(belongs_to(User, foreign_key = user))]
#[diesel(belongs_to(Challenge, foreign_key = challenge))]
#[diesel(table_name = artifacts)]
pub struct Artifact {
    pub id: Option<i32>,
    pub user: Option<i32>,
    pub challenge: i32,
    #[validate(length(min = 1))]
    pub flag: String,
    pub info: Json<Vec<ArtifactInfo>>,
    #[validate(length(min = 1))]
    pub path: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Associations,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(belongs_to(User, foreign_key = user))]
#[diesel(belongs_to(Challenge, foreign_key = challenge))]
#[diesel(table_name = submissions)]
pub struct Submission {
    pub id: Option<i32>,
    pub user: i32,
    pub challenge: i32,
    pub flag: String,
    pub time: PrimitiveDateTime,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Associations,
    Identifiable,
    Selectable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(belongs_to(User, foreign_key = user))]
#[diesel(belongs_to(Challenge, foreign_key = challenge))]
#[diesel(table_name = scores)]
pub struct Score {
    pub id: Option<i32>,
    pub user: i32,
    pub challenge: i32,
    pub time: PrimitiveDateTime,
    #[validate(range(min = 0.0))]
    pub points: f64,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Queryable,
    Associations,
    Selectable,
    Identifiable,
    AsChangeset,
    Validate,
)]
#[serde(crate = "rocket::serde")]
#[diesel(belongs_to(Submission, foreign_key = submission))]
#[diesel(table_name = solved)]
pub struct Solved {
    pub id: Option<i32>,
    pub submission: i32,
    pub score: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedSolved {
    #[serde(flatten)]
    pub submission: Submission,
    #[serde(flatten)]
    pub score: Score,
    #[serde(flatten)]
    pub solved: Solved,
}
