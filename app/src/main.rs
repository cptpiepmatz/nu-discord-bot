use axum::{response::IntoResponse, routing::get, Json, Router};
use serde_json::json;

async fn version() -> impl IntoResponse {
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {

    let router = Router::new()
        .route("/", get(version))
        .route("/version", get(version));

    Ok(router.into())
}