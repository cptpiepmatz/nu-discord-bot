use std::{collections::VecDeque, env, process::Command};

const TARGET: &str = "wasm32-wasip2";
const TARGET_DIR: &str = "target/wasm";

fn main() {
    build_wasm();
}

fn build_wasm() {
    let command = format!("cargo build --lib --target {TARGET} --target-dir {TARGET_DIR}");

    let mut args: VecDeque<_> = command.split(' ').collect();
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
        panic!("Could not build WASM library");
    }
}
