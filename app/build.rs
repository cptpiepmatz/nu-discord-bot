use std::{collections::VecDeque, env, fs, process::Command};

use wasmtime::component::Component;

const TARGET: &str = "wasm32-wasip2";
const TARGET_DIR: &str = "target/wasm";
const PACKAGE: &str = "nu-discord-bot-wasm";

fn main() {
    build_wasm();
}

fn build_wasm() {
    println!("cargo:rerun-if-changed=../wit/nu.wit");
    println!("cargo:rerun-if-changed=../wasm/src/lib.rs");
    println!("cargo:rerun-if-changed=../wasm/Cargo.toml");
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=../Cargo.lock");

    let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let profile = env::var("PROFILE").expect("set by cargo");
    let out_dir = env::var("OUT_DIR").expect("set by cargo");

    let command = format!(
        "cargo build --lib --package {PACKAGE} --target {TARGET} --target-dir {TARGET_DIR}"
    );

    let mut args: VecDeque<_> = command.split(' ').collect();
    let command = args.pop_front().unwrap();

    if profile == "release" {
        args.push_back("--release");
    }

    let status = Command::new(command)
        .args(&args)
        .current_dir(format!("{cargo_manifest_dir}/.."))
        .status()
        .unwrap();

    if !status.success() {
        panic!("Could not build WASM library");
    }

    let engine = common::make_engine().unwrap();
    let component_path = format!(
        "{cargo_manifest_dir}/../target/wasm/wasm32-wasip2/{profile}/nu_discord_bot_wasm.wasm"
    );
    let component =
        Component::from_file(&engine, component_path).expect("could not compile component");
    let component = component
        .serialize()
        .expect("could not serialize component");
    fs::write(out_dir + "/nu_discord_bot_wasm.wasm.bin", component)
        .expect("could not write serialized component");
}
