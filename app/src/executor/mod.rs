use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

use anyhow::{anyhow, Context};
use bytes::Bytes;
use wasmtime::{component::{Component, Linker}, Engine, Store};
use wasmtime_wasi::{IoView, ResourceTable, WasiCtx, WasiView};

wasmtime::component::bindgen!(in "../wit");

#[cfg(debug_assertions)]
static WASM_BYTES: &[u8] =
    include_bytes!("../../../target/wasm/wasm32-wasip2/debug/nu_discord_bot_wasm.wasm");
#[cfg(not(debug_assertions))]
static WASM_BYTES: &[u8] =
    include_bytes!("../../../target/wasm/wasm32-wasip2/release/nu_discord_bot_wasm.wasm");

pub struct NuExecutor {
    wasm_ready: Arc<AtomicBool>,
    execute_rx: tokio::sync::mpsc::Receiver<ExecuteParams>,
}

pub struct ExecuteParams {
    pub result_tx: tokio::sync::oneshot::Sender<ExecuteResult>,
    pub fname: String,
    pub source: String,
    pub file: Option<ExecuteParamsFile>,
}

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

    pub async fn run(mut self) -> anyhow::Result<crate::Never> {
        let ctx = Ctx {
            table: ResourceTable::new(),
            ctx: WasiCtx::builder().build(),
        };

        let engine = Engine::default();
        let component = Component::from_binary(&engine, WASM_BYTES).context("could not compile component")?;

        let mut store = Store::new(&engine, ctx);
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker).context("could not link against wasi")?;

        let world = Bot::instantiate(&mut store, &component, &mut linker).context("could not instantiate world")?;
        let guest = world.nu_discord_bot_nu().executor();

        let executor = guest.call_constructor(&mut store).context("could not construct executor")?;

        self.wasm_ready.store(true, Ordering::Relaxed);

        while let Some(params) = self.execute_rx.recv().await {
            let ExecuteParams {
                fname,
                source,
                file,
                result_tx
            } = params;
            let res = guest.call_execute(&mut store, executor, &fname, &source, file.map(Into::into).as_ref());
            result_tx.send(res).map_err(|_| anyhow!("could not send execute results"))?;
        }

        panic!("Nu Executor stopped.");
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
