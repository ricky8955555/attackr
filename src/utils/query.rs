use diesel::QueryResult;

pub trait QueryResultExt<T> {
    fn some(self) -> QueryResult<Option<T>>;
}

impl<T> QueryResultExt<T> for QueryResult<T> {
    fn some(self) -> QueryResult<Option<T>> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(err) => {
                if err == diesel::NotFound {
                    return Ok(None);
                }
                Err(err)
            }
        }
    }
}
