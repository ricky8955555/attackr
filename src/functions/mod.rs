pub mod challenge;
pub mod event;
pub mod user;

use rocket::fairing::AdHoc;

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Functions", |rocket| async {
        rocket.attach(challenge::stage()).attach(user::stage())
    })
}
