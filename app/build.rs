use std::{env, process::Command};

fn main() {
    if env::var("CARGO_FEATURE_SHUTTLE").is_ok() {
        let status = Command::new("cargo")
            .args([
                "build",
                "--lib",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir(env::var("CARGO_MANIFEST_DIR").unwrap() + "/..")
            .status()
            .unwrap();
        
        if !status.success() {
            panic!("Could not build WASM library")
        }
    }
}
