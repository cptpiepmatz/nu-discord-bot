use anyhow::{Context, anyhow};
use ed25519_dalek::VerifyingKey;
use middleware::VERIFY_KEY;
use static_toml::static_toml;
use twilight_model::id::{Id, marker::ApplicationMarker};

mod handlers;
mod middleware;
mod routes;
mod nu;

static_toml! {
    const CONSTANTS = include_toml!("../Constants.toml");
}

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(CONSTANTS.discord.application_id as u64);
const PUBLIC_KEY: [u8; 32] =
    match const_hex::const_decode_to_array(CONSTANTS.discord.public_key.as_bytes()) {
        Ok(value) => value,
        Err(_) => panic!("invalid public key"),
    };

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    VERIFY_KEY
        .set(VerifyingKey::from_bytes(&PUBLIC_KEY).context("invalid verifying key")?)
        .map_err(|_| anyhow!("verify key somehow already set"))?;

    let router = routes::create_router();
    Ok(router.into())
}
