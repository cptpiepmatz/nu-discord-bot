use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, anyhow};
use bytes::Bytes;
use exports::nu::discord_bot::nu::{ExecuteError, ExecuteOk};
use tracing::{debug, info, instrument};
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::{ResourceTable, p2::{IoView, WasiCtx, WasiView}};

use crate::error_and_bail;

wasmtime::component::bindgen!({
    path: "../wit",
    async: true,
});

#[cfg(debug_assertions)]
static WASM_BYTES: &[u8] =
    include_bytes!("../../../target/wasm/wasm32-wasip2/debug/nu_discord_bot_wasm.wasm");
#[cfg(not(debug_assertions))]
static WASM_BYTES: &[u8] =
    include_bytes!("../../../target/wasm/wasm32-wasip2/release/nu_discord_bot_wasm.wasm");

#[derive(Debug)]
pub struct NuExecutor {
    wasm_ready: Arc<AtomicBool>,
    execute_rx: tokio::sync::mpsc::Receiver<ExecuteParams>,
}

#[derive(Debug)]
pub struct ExecuteParams {
    pub result_tx: tokio::sync::oneshot::Sender<ExecuteResult>,
    pub fname: String,
    pub source: String,
    pub file: Option<ExecuteParamsFile>,
}

#[derive(Debug)]
pub enum ExecuteParamsFile {
    Bytes(Bytes),
    Text(String),
}

impl From<ExecuteParamsFile> for exports::nu::discord_bot::nu::File {
    fn from(value: ExecuteParamsFile) -> Self {
        use exports::nu::discord_bot::nu::File;
        match value {
            ExecuteParamsFile::Bytes(bytes) => File::Bytes(bytes.to_vec()),
            ExecuteParamsFile::Text(text) => File::Text(text),
        }
    }
}

pub type ExecuteResult = anyhow::Result<String>;

impl NuExecutor {
    pub fn new(
        wasm_ready: Arc<AtomicBool>,
        execute_rx: tokio::sync::mpsc::Receiver<ExecuteParams>,
    ) -> Self {
        Self {
            wasm_ready,
            execute_rx,
        }
    }

    #[instrument(name = "executor", skip_all)]
    pub async fn run(mut self) -> anyhow::Result<crate::Never> {
        let ctx = Ctx {
            table: ResourceTable::new(),
            ctx: WasiCtx::builder().build(),
        };

        let mut config = Config::default();
        let config = config.async_support(true);
        let engine = Engine::new(config).context("could not create engine")?;

        debug!("Compiling component");
        let component =
            Component::from_binary(&engine, WASM_BYTES).context("could not compile component")?;

        let mut store = Store::new(&engine, ctx);
        let mut linker = Linker::new(&engine);
        debug!("Linking WASI");
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).context("could not link against wasi")?;

        let world = Bot::instantiate_async(&mut store, &component, &mut linker)
            .await
            .context("could not instantiate world")?;
        let guest = world.nu_discord_bot_nu().executor();

        let executor = guest
            .call_constructor(&mut store)
            .await
            .context("could not construct executor")?;

        self.wasm_ready.store(true, Ordering::Relaxed);
        info!("WASM ready");

        while let Some(params) = self.execute_rx.recv().await {
            let ExecuteParams {
                fname,
                source,
                file,
                result_tx,
            } = params;
            let res = guest
                .call_execute(
                    &mut store,
                    executor,
                    &fname,
                    &source,
                    file.map(Into::into).as_ref(),
                )
                .await;
            #[rustfmt::skip]
            let res = match res {
                Ok(Ok(ExecuteOk::Value(ok) | ExecuteOk::Error(ok))) => Ok(ok),
                Ok(Err(ExecuteError::MergeDelta)) => Err(anyhow!("error while merging delta")),
                Ok(Err(ExecuteError::IntoValue)) => Err(anyhow!("error while turning res into value")),
                Ok(Err(ExecuteError::IntoString)) => Err(anyhow!("error while turning res into string")),
                Err(err) => Err(err),
            };
            result_tx
                .send(res)
                .map_err(|_| anyhow!("could not send execute results"))?;
        }

        error_and_bail!("Nu Executor stopped");
    }
}

struct Ctx {
    table: ResourceTable,
    ctx: WasiCtx,
}

impl IoView for Ctx {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiView for Ctx {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}
