pub mod challenge;

use std::{collections::HashMap, fs::File, io::Read, sync::LazyLock};

use anyhow::Result;
use koto::runtime::KValue;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::{configs::activity::CONFIG, utils::script::KotoScript};

static SCRIPTS: LazyLock<RwLock<HashMap<String, Mutex<KotoScript>>>> = LazyLock::new(|| {
    let mut scripts = HashMap::new();

    for info in CONFIG.scripts.iter() {
        let mut file = File::open(&info.path).unwrap();
        let mut code = String::new();
        file.read_to_string(&mut code).unwrap();

        let script = KotoScript::compile(&code).unwrap();
        scripts.insert(info.path.clone(), Mutex::new(script));
    }

    RwLock::new(scripts)
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActivityKind {
    Solved,
}

impl ActivityKind {
    fn function_name(&self) -> &'static str {
        match self {
            ActivityKind::Solved => "solved",
        }
    }
}

fn as_koto_value<T: Serialize>(value: T) -> Result<KValue> {
    let json = serde_json::to_value(value)?;
    Ok(koto_json::json_value_to_koto_value(&json)?)
}

async fn broadcast<'a>(kind: ActivityKind, args: &[KValue]) {
    let scripts = SCRIPTS.read().await;

    for info in CONFIG.scripts.iter() {
        if info.kinds.contains(&kind) {
            let mut script = scripts
                .get(&info.path)
                .expect("script should exists here.")
                .lock()
                .await;

            let function = kind.function_name();

            if let Err(e) = script.call_function(function, args) {
                log::error!(target: "activity", "call function {function} on {} failed: {e:?}", info.path);
            }
        }
    }
}
