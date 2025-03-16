use crate::handlers::discord;
use crate::middleware::validate_security_headers;
use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};

pub fn create_router() -> Router {
    Router::new()
        .route("/", get(info))
        .route("/info", get(info))
        .route(
            "/discord",
            post(discord).layer(from_fn(validate_security_headers)),
        )
}

async fn info() -> impl axum::response::IntoResponse {
    use axum::Json;
    use serde_json::json;

    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "application_id": crate::APPLICATION_ID,
        "public_key": crate::CONSTANTS.discord.public_key,
    }))
}
