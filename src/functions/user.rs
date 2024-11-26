use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rocket::{fairing::AdHoc, http::CookieJar, Build, Rocket};
use tokio::sync::RwLock;
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

const CHECK_CYCLE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct Session {
    id: i32,
    expiry: Instant,
}

static SESSIONS: LazyLock<RwLock<HashMap<String, Session>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

async fn session_check() {
    loop {
        {
            let mut sessions = SESSIONS.write().await;

            let now = Instant::now();
            let mut expired = Vec::new();

            for (id, session) in sessions.iter() {
                if now >= session.expiry {
                    expired.push(id.clone());
                }
            }

            for id in expired {
                sessions.remove(&id);
            }
        }

        tokio::time::sleep(CHECK_CYCLE).await;
    }
}

pub async fn initialize(rocket: Rocket<Build>) -> Rocket<Build> {
    tokio::spawn(session_check());

    rocket
}

async fn create_session(user: &User) -> Result<String> {
    if !user.enabled {
        bail!("disabled user.");
    }

    let expiry = Instant::now() + CONFIG.session.expiry;
    let session = Session {
        id: user.id.unwrap(),
        expiry,
    };
    let id = Uuid::new_v4().as_simple().to_string();

    SESSIONS.write().await.insert(id.clone(), session);

    Ok(id)
}

pub async fn new_session(jar: &CookieJar<'_>, user: &User) -> Result<()> {
    let session = create_session(user).await?;
    jar.add(("session", session));
    Ok(())
}

pub async fn destroy_session(jar: &CookieJar<'_>) -> Result<()> {
    let cookie = jar.get("session").ok_or(anyhow!("unauthorized session."))?;
    SESSIONS.write().await.remove(cookie.value());
    jar.remove("session");
    Ok(())
}

pub async fn auth_session(db: &Db, jar: &CookieJar<'_>) -> Result<User> {
    let cookie = jar.get("session").ok_or(anyhow!("unauthorized session."))?;
    let sessions = SESSIONS.read().await;
    let session = sessions
        .get(cookie.value())
        .ok_or(anyhow!("invalid or expired session."))?;

    let user = get_user(db, session.id).await?;

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
        rocket
            .attach(AdHoc::on_ignite(
                "Initialize Superuser",
                initialize_superuser,
            ))
            .attach(AdHoc::on_ignite("Initialize User Function", initialize))
    })
}
