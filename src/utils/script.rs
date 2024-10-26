use anyhow::{anyhow, Result};
use koto::prelude::*;

pub struct KotoScript {
    koto: Koto,
}

impl KotoScript {
    pub fn compile(script: &str) -> Result<Self> {
        let mut koto = Koto::new();
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
