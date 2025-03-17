use exports::nu::discord_bot::nu::{Guest, GuestExecutor};

wit_bindgen::generate!(in "../wit");

struct NuComponent;

impl Guest for NuComponent {
    type Executor = Executor;
}

struct Executor;

impl GuestExecutor for Executor {
    fn new() -> Self {
        Self
    }

    fn execute(&self, fname: String, source: String, file: Option<Vec<u8>>) -> String {
        format!("trying to execute: {source}")
    }
}

export!(NuComponent);
