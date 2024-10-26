use rocket::{fairing::AdHoc, http::Status, Request};
use rocket_dyn_templates::{context, Template};

#[catch(default)]
fn error_handler(status: Status, _: &Request) -> Template {
    Template::render("error", context! {status})
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Pages - Error", |rocket| async {
        rocket.register("/", catchers![error_handler])
    })
}
