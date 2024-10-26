mod admin;
mod core;
mod error;

use std::fmt::Display;

use rocket::{
    fairing::AdHoc,
    http::{uri::Reference, CookieJar},
    response::{Flash, Redirect},
};
use rocket_dyn_templates::{minijinja, Template};

use crate::{
    configs,
    db::{models::User, Db},
    utils,
};

async fn auth_session(db: &Db, jar: &CookieJar<'_>) -> Result<User> {
    crate::functions::user::auth_session(db, jar)
        .await
        .flash_expect(uri!(core::user::ROOT, core::user::login_page), "认证失败")
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Pages", |rocket| async {
        rocket
            .attach(Template::custom(|engines| {
                engines.minijinja.add_global(
                    "event",
                    minijinja::Value::from_serialize(&*configs::event::CONFIG),
                );
                engines.minijinja.add_filter("sumint", utils::jinja::sum);
                engines.minijinja.set_trim_blocks(true);
                engines.minijinja.set_lstrip_blocks(true);
            }))
            .attach(admin::stage())
            .attach(core::stage())
            .attach(error::stage())
    })
}

pub type Result<T> = std::result::Result<T, Error>;

#[allow(clippy::large_enum_variant)]
#[derive(Responder)]
pub enum Error {
    Page(Template),
    Redirect(Flash<Redirect>),
}

impl Error {
    pub fn redirect<U: TryInto<Reference<'static>>>(uri: U, msg: &str) -> Self {
        Self::Redirect(Flash::error(Redirect::to(uri), msg))
    }
}

pub trait ResultFlashExt<T, E> {
    #[allow(dead_code)]
    fn flash_unwrap<U: TryInto<Reference<'static>>>(self, uri: U) -> Result<T>;
    fn flash_expect<U: TryInto<Reference<'static>>>(self, uri: U, msg: &str) -> Result<T>;
}

impl<T, E> ResultFlashExt<T, E> for std::result::Result<T, E>
where
    E: Display,
{
    fn flash_unwrap<U: TryInto<Reference<'static>>>(self, uri: U) -> Result<T> {
        self.map_err(|err| Error::redirect(uri, &format!("{err}")))
    }

    fn flash_expect<U: TryInto<Reference<'static>>>(self, uri: U, msg: &str) -> Result<T> {
        self.map_err(|err| Error::redirect(uri, &format!("{msg}: {err}")))
    }
}
