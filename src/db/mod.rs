pub mod models;
pub mod query;
pub mod schema;
pub mod types;

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::{fairing::AdHoc, Build, Rocket};

#[rocket_sync_db_pools::database("database")]
pub struct Db(diesel::SqliteConnection);

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    Db::get_one(&rocket)
        .await
        .expect("database connection")
        .run(|conn| {
            conn.run_pending_migrations(MIGRATIONS)
                .expect("failed to run migrations");
        })
        .await;

    rocket
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Database", |rocket| async {
        rocket
            .attach(Db::fairing())
            .attach(AdHoc::on_ignite("Database Migrations", run_migrations))
    })
}
