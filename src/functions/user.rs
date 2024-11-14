use std::{fs, sync::LazyLock, time::SystemTime};

use anyhow::{anyhow, bail, Result};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rocket::{fairing::AdHoc, http::CookieJar, Build, Rocket};
use serde::{Deserialize, Serialize};

use crate::{
    configs::user::{Key, CONFIG},
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

static ACCESS_TOKEN_PRIVKEY: LazyLock<EncodingKey> = LazyLock::new(|| match &CONFIG.jwt.key {
    Key::KeyPair { privkey, pubkey: _ } => {
        let key = &fs::read(privkey).unwrap();
        match CONFIG.jwt.algo {
            Algorithm::ES256 | Algorithm::ES384 => EncodingKey::from_ec_pem(key).unwrap(),
            Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512
            | Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512 => EncodingKey::from_rsa_pem(key).unwrap(),
            Algorithm::EdDSA => EncodingKey::from_ed_pem(key).unwrap(),
            _ => panic!(
                "key pair is not applicable to algorithm {:?}",
                CONFIG.jwt.algo
            ),
        }
    }
    Key::Passphrase { passphrase } => match CONFIG.jwt.algo {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            EncodingKey::from_secret(passphrase.as_bytes())
        }
        _ => panic!(
            "passphrase is not applicable to algorithm {:?}",
            CONFIG.jwt.algo
        ),
    },
});

static ACCESS_TOKEN_PUBKEY: LazyLock<DecodingKey> = LazyLock::new(|| match &CONFIG.jwt.key {
    Key::KeyPair { privkey: _, pubkey } => {
        let key = &fs::read(pubkey).unwrap();
        match CONFIG.jwt.algo {
            Algorithm::ES256 | Algorithm::ES384 => DecodingKey::from_ec_pem(key).unwrap(),
            Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512
            | Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512 => DecodingKey::from_rsa_pem(key).unwrap(),
            Algorithm::EdDSA => DecodingKey::from_ed_pem(key).unwrap(),
            _ => panic!(
                "key pair is not applicable to algorithm {:?}",
                CONFIG.jwt.algo
            ),
        }
    }
    Key::Passphrase { passphrase } => match CONFIG.jwt.algo {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            DecodingKey::from_secret(passphrase.as_bytes())
        }
        _ => panic!(
            "passphrase is not applicable to algorithm {:?}",
            CONFIG.jwt.algo
        ),
    },
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessToken {
    id: i32,
    random: String,
    exp: u64,
}

fn create_token(user: &User) -> Result<String> {
    if !user.enabled {
        bail!("disabled user.");
    }

    let exp = (SystemTime::now() + CONFIG.jwt.expiry)
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time should not be earlier than UNIX_EPOCH.")
        .as_secs();

    let claim = AccessToken {
        id: user.id.unwrap(),
        random: user.random.clone(),
        exp,
    };

    Ok(encode(
        &Header::new(CONFIG.jwt.algo),
        &claim,
        &ACCESS_TOKEN_PRIVKEY,
    )?)
}

pub fn new_session(jar: &CookieJar<'_>, user: &User) -> Result<()> {
    let token = create_token(user)?;
    jar.add(("token", token));
    Ok(())
}

pub fn destroy_session(jar: &CookieJar<'_>) {
    jar.remove("token");
}

fn verify_token(token: &str) -> Result<AccessToken> {
    let token = decode::<AccessToken>(
        token,
        &ACCESS_TOKEN_PUBKEY,
        &Validation::new(CONFIG.jwt.algo),
    )?;

    Ok(token.claims)
}

pub async fn auth_session(db: &Db, jar: &CookieJar<'_>) -> Result<User> {
    let jwt = jar.get("token").ok_or(anyhow!("unauthorized session."))?;
    let token = verify_token(jwt.value())?;
    let user = get_user(db, token.id).await?;

    if !user.enabled {
        bail!("disabled user.");
    }

    if token.random != user.random {
        bail!("random not match.");
    }

    Ok(user)
}

pub fn hash_password(password: &str) -> String {
    hash(password, DEFAULT_COST).unwrap()
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    Ok(verify(password, hash)?)
}

pub fn generate_random() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
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
            password: hash_password(&password),
            contact: "admin".to_string(),
            email: "admin@example.com".to_string(),
            enabled: true,
            role: UserRole::Superuser,
            nickname: None,
            random: generate_random(),
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
