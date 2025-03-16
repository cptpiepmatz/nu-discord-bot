use anyhow::{Context, anyhow};
use ed25519_dalek::VerifyingKey;
use hex_literal::hex;
use middleware::VERIFY_KEY;
use twilight_model::id::{Id, marker::ApplicationMarker};

mod handlers;
mod middleware;
mod routes;

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(1350440050927865878);
const PUBLIC_KEY: [u8; 32] =
    hex!("cc5d283a4aa8d726bcbd7938bfccda57984568d5e7b8546ac963a7cfd2fa0b5e");

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    VERIFY_KEY
        .set(VerifyingKey::from_bytes(&PUBLIC_KEY).context("invalid verifying key")?)
        .map_err(|_| anyhow!("verify key somehow already set"))?;

    let router = routes::create_router();
    Ok(router.into())
}
