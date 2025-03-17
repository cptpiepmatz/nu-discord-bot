use bytes::Bytes;
use wasmtime::{
    Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::{
    IoImpl, IoView, ResourceTable, WasiCtx, WasiImpl, WasiView,
    bindings::{cli::exit::LinkOptions, exports::wasi},
};

wasmtime::component::bindgen!(in "../wit");

#[cfg(debug_assertions)]
static CWASM_BYTES: &[u8] =
    include_bytes!("../../target/wasm/wasm32-wasip2/debug/nu_discord_bot_wasm.cwasm");
#[cfg(not(debug_assertions))]
static CWASM_BYTES: &[u8] =
    include_bytes!("../../target/wasm/wasm32-wasip2/release/nu_discord_bot_wasm.cwasm");

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

pub struct ExecuteParams {
    pub fname: String,
    pub source: String,
    pub file: Option<Bytes>,
    pub res_tx: tokio::sync::oneshot::Sender<String>,
}

pub async fn run(mut rx: tokio::sync::mpsc::Receiver<ExecuteParams>) -> ! {
    let ctx = Ctx {
        table: ResourceTable::new(),
        ctx: WasiCtx::builder().build(),
    };

    let engine = Engine::default();
    let component = unsafe { Component::deserialize(&engine, CWASM_BYTES) }
        .expect("could not deserialize component");

    let mut store = Store::new(&engine, ctx);
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).expect("could not link against wasi");

    let world =
        Bot::instantiate(&mut store, &component, &mut linker).expect("could not instantiate world");
    let guest = world.nu_discord_bot_nu().executor();

    let executor = guest
        .call_constructor(&mut store)
        .expect("could not construct executor");

    loop {
        let params = rx.recv().await.expect("execution sender died");
        let ExecuteParams {
            fname,
            source,
            file,
            res_tx,
        } = params;
        let res = guest
            .call_execute(&mut store, executor, &fname, &source, file.as_deref())
            .expect("execute failed");
        res_tx.send(res).expect("sending execute response failed");
    }
}
