use std::sync::LazyLock;

use anyhow::{anyhow, bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use moka::future::Cache;
use rocket::{fairing::AdHoc, http::CookieJar, Build, Rocket};
use uuid::Uuid;

use crate::{
    configs::user::CONFIG,
    db::{
        models::{User, UserRole},
        query::{
            artifact::list_user_artifacts,
            user::{add_user, delete_user, get_user, list_users},
        },
        Db,
    },
    functions::challenge::clear_artifact,
};

static SESSIONS: LazyLock<Cache<String, i32>> = LazyLock::new(|| {
    Cache::builder()
        .support_invalidation_closures()
        .time_to_live(CONFIG.session.expiry)
        .build()
});

async fn create_session(user: &User) -> Result<String> {
    if !user.enabled {
        bail!("disabled user.");
    }

    let id = Uuid::new_v4().as_simple().to_string();
    SESSIONS.insert(id.clone(), user.id.unwrap()).await;

    Ok(id)
}

pub async fn new_session(jar: &CookieJar<'_>, user: &User) -> Result<()> {
    let session = create_session(user).await?;
    jar.add(("session", session));
    Ok(())
}

pub async fn destroy_session(jar: &CookieJar<'_>) -> Result<()> {
    let cookie = jar.get("session").ok_or(anyhow!("unauthorized session."))?;
    SESSIONS.invalidate(cookie.value()).await;
    jar.remove("session");
    Ok(())
}

pub fn invalidate_user_sessions(user: i32) {
    SESSIONS
        .invalidate_entries_if(move |_, v| v == &user)
        .expect("invalidation closure enabled");
}

pub async fn auth_session(db: &Db, jar: &CookieJar<'_>) -> Result<User> {
    let cookie = jar.get("session").ok_or(anyhow!("unauthorized session."))?;

    let id = SESSIONS
        .get(cookie.value())
        .await
        .ok_or(anyhow!("invalid or expired session."))?;

    let user = get_user(db, id).await?;

    if !user.enabled {
        bail!("disabled user.");
    }

    Ok(user)
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let hash = PasswordHash::new(hash)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .is_ok())
}

pub async fn remove_user(db: &Db, id: i32) -> Result<()> {
    let artifacts = list_user_artifacts(db, id).await?;

    for artifact in artifacts {
        _ = clear_artifact(&artifact).await;
    }

    delete_user(db, id).await?;

    Ok(())
}

pub fn is_admin(user: &User) -> bool {
    user.role >= UserRole::Administrator
}

pub async fn initialize_superuser(rocket: Rocket<Build>) -> Rocket<Build> {
    let db = Db::get_one(&rocket).await.expect("database connection");

    let users = list_users(&db).await.expect("failed to list users.");

    if users.is_empty() {
        let username = "admin";
        let password = uuid::Uuid::new_v4().simple().to_string();

        let user = User {
            id: None,
            username: username.to_string(),
            password: hash_password(&password).expect("failed to hash password"),
            contact: "admin".to_string(),
            email: "admin@example.com".to_string(),
            enabled: true,
            role: UserRole::Superuser,
            nickname: None,
        };

        add_user(&db, user).await.expect("failed to add superuser.");

        log::info!("Superuser created: username '{username}' password '{password}'.")
    }

    rocket
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Function - User", |rocket| async {
        rocket.attach(AdHoc::on_ignite(
            "Initialize Superuser",
            initialize_superuser,
        ))
    })
}
