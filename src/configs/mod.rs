pub mod challenge;
pub mod event;
pub mod user;

use std::{fs::File, io::Read, path::Path};

use serde::de::DeserializeOwned;
use validator::Validate;

fn load_config<P, T>(path: P) -> T
where
    P: AsRef<Path>,
    T: DeserializeOwned + Validate,
{
    let path = Path::new("configs").join(&path).with_extension("yml");

    let mut buf = Vec::new();

    if path.exists() {
        let mut fp = File::open(path).expect("open file");
        fp.read_to_end(&mut buf).expect("read file");
    }

    let config: T = serde_yml::from_slice(&buf).expect("deserialize");
    config.validate().unwrap();

    config
}
