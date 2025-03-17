use std::borrow::Cow;

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
        let file: Cow<_> = match file {
            Some(file) => match String::from_utf8(file) {
                Ok(file) => file.into(),
                Err(_) => "not utf-8".into(),
            },
            None => "no file".into(),
        };

        format!("trying to execute: \u{001b}[0;32m{source}\u{001b}[0;0m\n{file}")
    }
}

export!(NuComponent);
