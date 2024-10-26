use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use diesel::{
    expression::AsExpression,
    serialize::{IsNull, ToSql},
    sql_types,
    sqlite::Sqlite,
    Queryable,
};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, AsExpression)]
#[diesel(sql_type = sql_types::Text)]
pub struct Json<T>(pub T);

impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Serialize for Json<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Json<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(T::deserialize(deserializer)?))
    }
}

impl<T> Queryable<sql_types::Text, Sqlite> for Json<T>
where
    T: DeserializeOwned,
{
    type Row = String;

    fn build(row: Self::Row) -> diesel::deserialize::Result<Self> {
        Ok(serde_json::from_str(&row)?)
    }
}

impl<T> ToSql<sql_types::Text, Sqlite> for Json<T>
where
    T: Serialize + Debug,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        let json = serde_json::to_string(self)?;
        out.set_value(json);

        Ok(IsNull::No)
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Json<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}
