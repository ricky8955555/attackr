#[cfg(feature = "koto_exec")]
use {koto::runtime::ErrorKind, std::process::Command};

use anyhow::{anyhow, Result};
use koto::prelude::*;

#[cfg(feature = "koto_exec")]
fn exec(ctx: &mut CallContext) -> koto::Result<KValue> {
    match ctx.args() {
        [KValue::Str(program), KValue::Tuple(targs)] => {
            let mut args = Vec::with_capacity(targs.len());

            for arg in targs.iter() {
                match arg {
                    KValue::Str(val) => args.push(val.as_str()),
                    unexpected => return type_error("expected str type arg.", unexpected),
                }
            }

            let output = Command::new(program.as_str())
                .args(args)
                .output()
                .map_err(|err| ErrorKind::StringError(format!("{err:?}")))?;

            let stdout = KValue::Str(String::from_utf8_lossy(&output.stdout).to_string().into());
            let stderr = KValue::Str(String::from_utf8_lossy(&output.stderr).to_string().into());

            Ok(KValue::Tuple((&[stdout, stderr]).into()))
        }
        unexpected => type_error_with_slice("expected program and args.", unexpected),
    }
}

pub struct KotoScript {
    koto: Koto,
}

impl KotoScript {
    pub fn compile(script: &str) -> Result<Self> {
        let mut koto = Koto::new();

        #[cfg(any(
            feature = "koto_exec",
            feature = "koto_json",
            feature = "koto_random",
            feature = "koto_tempfile"
        ))]
        let prelude = koto.prelude();

        #[cfg(feature = "koto_exec")]
        prelude.add_fn("exec", exec);

        #[cfg(feature = "koto_json")]
        prelude.insert("json", koto_json::make_module());
        #[cfg(feature = "koto_random")]
        prelude.insert("random", koto_random::make_module());
        #[cfg(feature = "koto_tempfile")]
        prelude.insert("tempfile", koto_tempfile::make_module());

        koto.compile_and_run(script)?;

        Ok(Self { koto })
    }

    pub fn get(&self, name: &str) -> Option<KValue> {
        let exported = self.koto.exports();
        exported.get(name)
    }

    pub fn call_function<'a, A>(&mut self, name: &str, args: A) -> Result<KValue>
    where
        A: Into<CallArgs<'a>>,
    {
        let function = self
            .get(name)
            .ok_or_else(|| anyhow!("function '{name}' not found."))?;

        let ret = self.koto.call_function(function, args)?;

        Ok(ret)
    }
}
