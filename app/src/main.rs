use std::{sync::OnceLock, usize};

use anyhow::{Context, anyhow};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hex_literal::hex;
use serde_json::json;
use twilight_model::{
    application::interaction::Interaction,
    id::{Id, marker::ApplicationMarker},
};

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(1350440050927865878);
const PUBLIC_KEY: [u8; 32] =
    hex!("cc5d283a4aa8d726bcbd7938bfccda57984568d5e7b8546ac963a7cfd2fa0b5e");

static VERIFY_KEY: OnceLock<VerifyingKey> = OnceLock::new();

async fn version() -> impl IntoResponse {
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn discord(payload: Json<Interaction>) -> impl IntoResponse {
    "" // TODO
}

async fn validate_security_headers(request: Request, next: Next) -> Result<Response, StatusCode> {
    let (parts, body) = request.into_parts();
    let headers = &parts.headers;

    let verify_key = VERIFY_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let signature = headers
        .get("X-Signature-Ed25519")
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let signature = hex::decode(signature).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let signature = <[u8; 64]>::try_from(signature).map_err(|_| StatusCode::BAD_REQUEST)?;
    let signature = Signature::from_bytes(&signature);

    let timestamp = headers
        .get("X-Signature-Timestamp")
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let body = to_bytes(body, 4096) // Set a reasonable limit
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let msg = [timestamp.as_bytes(), &body].concat();

    if verify_key.verify(&msg, &signature).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let request = Request::from_parts(parts, Body::from(body));
    Ok(next.run(request).await)
}

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    VERIFY_KEY
        .set(VerifyingKey::from_bytes(&PUBLIC_KEY).context("invalid verifying key")?)
        .map_err(|_| anyhow!("verify key somehow already set"))?;

    let router = Router::new()
        .route("/", get(version))
        .route("/version", get(version))
        .route(
            "/discord",
            post(discord).layer(axum::middleware::from_fn(validate_security_headers)),
        );

    Ok(router.into())
}
