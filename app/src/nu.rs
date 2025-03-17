use wasmtime::{
    Engine, Store,
    component::{Component, Linker},
};
use wasmtime_wasi::{bindings::{cli::exit::LinkOptions, exports::wasi}, IoImpl, IoView, ResourceTable, WasiCtx, WasiImpl, WasiView};

wasmtime::component::bindgen!(in "../wit");

#[test]
fn test_bindings() -> wasmtime::Result<()> {
    struct Ctx {
        table: ResourceTable,
        ctx: WasiCtx,
    }

    let ctx = Ctx {
        table: ResourceTable::new(),
        ctx: WasiCtx::builder().build(),
    };

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

    let engine = Engine::default();
    let component = unsafe {
        Component::deserialize(
            &engine,
            include_bytes!("../../target/wasm/wasm32-wasip2/debug/nu_discord_bot_wasm.cwasm"),
        )
    }?;

    let mut store = Store::new(&engine, ctx);
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    
    let world = Bot::instantiate(&mut store, &component, &mut linker)?;
    let guest = world.nu_discord_bot_nu().executor();

    let executor = guest.call_constructor(&mut store)?;
    let res = guest.call_execute(&mut store, executor, "fname", "source", None)?;

    assert_eq!(res, "trying to execute: source");

    Ok(())
}

// TODO: write async function that handles nu interactions
