pub mod artifact;
pub mod challenge;
pub mod difficulty;
pub mod problemset;
pub mod root;
pub mod submission;
pub mod user;

use std::fmt::Display;

use rocket::fairing::AdHoc;
use rocket_dyn_templates::{context, Template};

use crate::db::models::{User, UserRole};

use super::{Error, Result};

#[allow(clippy::result_large_err)]
#[inline]
fn check_permission(user: &User) -> Result<()> {
    if user.role < UserRole::Administrator {
        return Err(Error::redirect(
            uri!(super::core::user::ROOT, super::core::user::index),
            "禁止 Administrator 以下权限组访问管理界面或调用相关接口",
        ));
    }

    Ok(())
}

fn show_error(msg: &str) -> Template {
    Template::render("admin/error", context! {msg})
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

trait OptionResponseExt<T> {
    fn resp_expect(self, msg: &str) -> Result<T>;
}

impl<T> OptionResponseExt<T> for Option<T> {
    fn resp_expect(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| Error::Page(show_error(msg)))
    }
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Admin Pages", |rocket| async {
        rocket
            .attach(artifact::stage())
            .attach(challenge::stage())
            .attach(difficulty::stage())
            .attach(problemset::stage())
            .attach(root::stage())
            .attach(submission::stage())
            .attach(user::stage())
    })
}
