use std::{collections::VecDeque, env, process::Command};

const COMMAND: &str = "cargo build --lib --target wasm32-wasip2 --target-dir target/wasm";

fn main() {
    let mut args: VecDeque<_> = COMMAND.split(' ').collect();
    let command = args.pop_front().unwrap();

    if let Ok("release") = env::var("PROFILE").as_deref() {
        args.push_back("--release");
    }

    let status = Command::new(command)
        .args(&args)
        .current_dir(env::var("CARGO_MANIFEST_DIR").unwrap() + "/..")
        .status()
        .unwrap();

    if !status.success() {
        panic!("Could not build WASM library")
    }
}
