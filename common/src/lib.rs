use anyhow::Context;
use wasmtime::{Config, Engine};

pub fn make_engine() -> anyhow::Result<Engine> {
    let mut config = Config::default();
    let config = config.async_support(true);
    Engine::new(config).context("could not create engine")
}
