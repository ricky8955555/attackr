use rocket::{fairing::AdHoc, http::uri::Origin, request::FlashMessage};
use rocket_dyn_templates::{context, Template};

use crate::functions::event::{cmp_period, primitive_now};

#[allow(clippy::declare_interior_mutable_const)]
pub const ROOT: Origin<'static> = uri!("/");

#[get("/")]
fn index(flash: Option<FlashMessage<'_>>) -> Template {
    let period = cmp_period(primitive_now()) as i8;

    Template::render("core/index", context! {flash, period})
}

pub fn stage() -> AdHoc {
    let routes = routes![index];

    AdHoc::on_ignite("Core Pages - Root", |rocket| async {
        rocket.mount(ROOT, routes)
    })
}
