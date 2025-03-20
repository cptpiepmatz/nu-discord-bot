use std::sync::{Arc, atomic::AtomicBool};

use bytes::Bytes;

pub struct NuExecutor {
    wasm_ready: Arc<AtomicBool>,
    execute_rx: tokio::sync::mpsc::Receiver<ExecuteParams>,
}

pub struct ExecuteParams {
    pub result_tx: tokio::sync::oneshot::Sender<ExecuteResult>,
    pub source: String,
    pub file: Option<ExecuteParamsFile>,
}

pub enum ExecuteParamsFile {
    Bytes(Bytes),
    Text(String),
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

    pub async fn run(self) -> anyhow::Result<crate::Never> {
        panic!("Nu Executor stopped.");
    }
}
