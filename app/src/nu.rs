use std::time::Duration;

use bytes::Bytes;
use wasmtime::{
    Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::{IoView, ResourceTable, WasiCtx, WasiView};

wasmtime::component::bindgen!(in "../wit");

#[cfg(debug_assertions)]
static WASM_BYTES: &[u8] =
    include_bytes!("../../target/wasm/wasm32-wasip2/debug/nu_discord_bot_wasm.wasm");
#[cfg(not(debug_assertions))]
static WASM_BYTES: &[u8] =
    include_bytes!("../../target/wasm/wasm32-wasip2/release/nu_discord_bot_wasm.wasm");

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
    let component =
        Component::from_binary(&engine, WASM_BYTES).expect("could not compile component");

    let mut store = Store::new(&engine, ctx);
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).expect("could not link against wasi");

    let world =
        Bot::instantiate(&mut store, &component, &mut linker).expect("could not instantiate world");
    let guest = world.nu_discord_bot_nu().executor();

    let executor = guest
        .call_constructor(&mut store)
        .expect("could not construct executor");

    println!("wasm loaded");

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
        dbg!(&res);
        res_tx.send(res).expect("sending execute response failed");
    }
}

#[tokio::test]
async fn test_run() {
    let req = tokio::sync::mpsc::channel(1);
    let run_handle = tokio::spawn(run(req.1));

    let res = tokio::sync::oneshot::channel();
    req.0
        .send(ExecuteParams {
            fname: "test".into(),
            source: "help commands".into(),
            file: None,
            res_tx: res.0,
        })
        .await
        .unwrap();

    let res = res.1.await.unwrap();
    assert!(!res.is_empty());

    run_handle.abort();
}
