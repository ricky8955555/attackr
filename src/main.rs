#[macro_use]
extern crate rocket;

#[cfg(feature = "activity")]
mod activity;
mod configs;
mod core;
mod db;
mod functions;
mod pages;
mod utils;

use rocket::fs::{FileServer, Options as FsOptions};

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(db::stage())
        .attach(functions::stage())
        .attach(pages::stage())
        .mount("/static", FileServer::new("static", FsOptions::None))
}
