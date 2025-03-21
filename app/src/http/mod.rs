use std::sync::Arc;

use anyhow::{Context, bail};
use axum::{
    Json,
    body::Body,
    extract::Request,
    middleware::{Next, from_fn},
    response::Response,
    routing::{get, post},
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::StatusCode;
use serde_json::json;
use twilight_model::{
    application::interaction::{Interaction, InteractionType},
    channel::message::embed::EmbedField,
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::{InteractionResponseDataBuilder, embed::EmbedBuilder};
use tracing::{error, instrument};

#[derive(Debug, Clone)]
pub struct HttpHandler {
    verification_key: VerifyingKey,
    interaction_tx: tokio::sync::mpsc::Sender<Interaction>,
}

impl HttpHandler {
    pub fn new(interaction_tx: tokio::sync::mpsc::Sender<Interaction>) -> anyhow::Result<Self> {
        Ok(Self {
            verification_key: VerifyingKey::from_bytes(&crate::PUBLIC_KEY)
                .context("invalid verifying key")?,
            interaction_tx,
        })
    }

    #[instrument(name = "http", skip_all)]
    pub async fn bind(self, addr: std::net::SocketAddr) -> anyhow::Result<crate::Never> {
        let http_handler = Arc::new(self);

        let http_handler_clone = Arc::clone(&http_handler);
        let discord_route = move |req| {
            let http_handler = Arc::clone(&http_handler_clone);
            async move { http_handler.discord(req).await }
        };

        let http_handler_clone = Arc::clone(&http_handler);
        let validate_security_headers = move |req, next| {
            let http_handler = Arc::clone(&http_handler_clone);
            async move { http_handler.validate_security_headers(req, next).await }
        };

        let router = axum::Router::new()
            .route("/", get(HttpHandler::info))
            .route("/info", get(HttpHandler::info))
            .route(
                "/discord",
                post(discord_route).layer(from_fn(validate_security_headers)),
            );

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        error!("HTTP Server stopped");
        bail!("HTTP Server stopped.")
    }

    async fn info() -> impl axum::response::IntoResponse {
        Json(json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "application_id": crate::APPLICATION_ID,
            "public_key": crate::CONSTANTS.discord.public_key,
        }))
    }

    async fn validate_security_headers(
        &self,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let (parts, body) = request.into_parts();
        let headers = &parts.headers;

        let signature = headers
            .get("X-Signature-Ed25519")
            .ok_or(StatusCode::BAD_REQUEST)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let signature =
            const_hex::decode(signature).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
        let signature = <[u8; 64]>::try_from(signature).map_err(|_| StatusCode::BAD_REQUEST)?;
        let signature = Signature::from_bytes(&signature);

        let timestamp = headers
            .get("X-Signature-Timestamp")
            .ok_or(StatusCode::BAD_REQUEST)?
            .to_str()
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let body = axum::body::to_bytes(body, 4096)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let msg = [timestamp.as_bytes(), &body].concat();

        if self.verification_key.verify(&msg, &signature).is_err() {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let request = Request::from_parts(parts, Body::from(body));
        Ok(next.run(request).await)
    }

    async fn discord(
        &self,
        interaction: Json<Interaction>,
    ) -> axum::response::Result<Json<InteractionResponse>> {
        match interaction.kind {
            InteractionType::Ping => Ok(HttpHandler::ping()),
            InteractionType::ApplicationCommand => Ok(self.application_command(interaction).await),
            InteractionType::MessageComponent
            | InteractionType::ModalSubmit
            | InteractionType::ApplicationCommandAutocomplete => {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "unimplemented").into())
            }
            _ => Err(StatusCode::UNPROCESSABLE_ENTITY.into()),
        }
    }

    fn ping() -> Json<InteractionResponse> {
        Json(InteractionResponse {
            kind: InteractionResponseType::Pong,
            data: None,
        })
    }

    async fn application_command(
        &self,
        interaction: Json<Interaction>,
    ) -> Json<InteractionResponse> {
        if let Err(err) = self.interaction_tx.send(interaction.0).await {
            let embed = EmbedBuilder::new()
                .title("⚠️ Interaction Handler Died")
                .description(format!(
                    "Could not send interaction to Interaction Handler.\nReport this to <@{}>.",
                    crate::SUPPORT_USER_ID
                ))
                .field(EmbedField {
                    inline: false,
                    name: std::any::type_name_of_val(&err).to_string(),
                    value: err.to_string(),
                })
                .color(crate::CONSTANTS.colors.red as u32)
                .build();

            return Json(InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: InteractionResponseDataBuilder::new()
                    .embeds([embed])
                    .build()
                    .into(),
            });
        };

        Json(InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: None,
        })
    }
}
