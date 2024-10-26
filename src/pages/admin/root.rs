use rocket::{
    fairing::AdHoc,
    http::{uri::Origin, CookieJar},
    request::FlashMessage,
};
use rocket_dyn_templates::{context, Template};

use crate::{
    db::Db,
    pages::{auth_session, Result},
};

use super::check_permission;

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/admin");

#[get("/")]
pub(super) async fn index(
    jar: &CookieJar<'_>,
    db: Db,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template> {
    let current = auth_session(&db, jar).await?;
    check_permission(&current)?;

    Ok(Template::render("admin/index", context! {flash}))
}

pub fn stage() -> AdHoc {
    let routes = routes![index];

    AdHoc::on_ignite("Admin Pages - Root", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
