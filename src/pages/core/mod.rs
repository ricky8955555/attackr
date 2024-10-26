pub mod challenge;
pub mod root;
pub mod scoreboard;
pub mod user;

use std::fmt::Display;

use rocket::fairing::AdHoc;
use rocket_dyn_templates::{context, Template};

use crate::{db::models::User, functions::event::is_available};

use super::{Error, Result};

#[allow(clippy::result_large_err)]
#[inline]
fn check_event_availability(user: Option<&User>) -> Result<()> {
    if !is_available(user) {
        return Err(Error::redirect(
            uri!(user::ROOT, user::index),
            "禁止在比赛开始前访问题目或调用相关接口",
        ));
    }

    Ok(())
}

fn show_error(msg: &str) -> Template {
    Template::render("core/error", context! {msg})
}

trait ResultResponseExt<T, E> {
    #[allow(dead_code)]
    fn resp_unwrap(self) -> Result<T>;
    fn resp_expect(self, msg: &str) -> Result<T>;
}

impl<T, E> ResultResponseExt<T, E> for std::result::Result<T, E>
where
    E: Display,
{
    fn resp_unwrap(self) -> Result<T> {
        self.map_err(|err| Error::Page(show_error(&format!("{err}"))))
    }

    fn resp_expect(self, msg: &str) -> Result<T> {
        self.map_err(|err| Error::Page(show_error(&format!("{msg}: {err}"))))
    }
}

pub trait OptionResponseExt<T> {
    #[allow(dead_code)]
    fn resp_expect(self, msg: &str) -> Result<T>;
}

impl<T> OptionResponseExt<T> for Option<T> {
    fn resp_expect(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| Error::Page(show_error(msg)))
    }
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Core Pages", |rocket| async {
        rocket
            .attach(challenge::stage())
            .attach(root::stage())
            .attach(scoreboard::stage())
            .attach(user::stage())
    })
}
