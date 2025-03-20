pub struct NuExecutor;

impl NuExecutor {
    pub fn new() -> Self {
        todo!();
    }

    pub async fn run(self) -> anyhow::Result<crate::Never> {
        panic!("Nu Executor stopped.");
    }
}
