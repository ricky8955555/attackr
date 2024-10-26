use rocket::{fairing::AdHoc, http::uri::Origin, request::FlashMessage};
use rocket_dyn_templates::{context, Template};
use time::OffsetDateTime;

use crate::functions::event::cmp_period;

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/");

#[get("/")]
fn index(flash: Option<FlashMessage<'_>>) -> Template {
    let period = cmp_period(OffsetDateTime::now_utc()) as i8;

    Template::render("core/index", context! {flash, period})
}

pub fn stage() -> AdHoc {
    let routes = routes![index];

    AdHoc::on_ignite("Core Pages - Root", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
