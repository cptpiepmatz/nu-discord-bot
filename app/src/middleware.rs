use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::sync::OnceLock;

pub static VERIFY_KEY: OnceLock<VerifyingKey> = OnceLock::new();

pub async fn validate_security_headers(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
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

    let body = to_bytes(body, 4096)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let msg = [timestamp.as_bytes(), &body].concat();

    if verify_key.verify(&msg, &signature).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let request = Request::from_parts(parts, Body::from(body));
    Ok(next.run(request).await)
}
