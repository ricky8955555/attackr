use rocket_dyn_templates::minijinja::{Error, ErrorKind, Value};

pub fn sum(value: Value) -> Result<Value, Error> {
    let iter = value.try_iter().map_err(|err| {
        Error::new(ErrorKind::InvalidOperation, "cannot convert value to list").with_source(err)
    })?;

    Ok(iter
        .map(f64::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map(|x| Value::from(x.into_iter().sum::<f64>()))
        .unwrap_or(Value::UNDEFINED))
}
