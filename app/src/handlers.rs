use anyhow::{Context, anyhow};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use twilight_http::client::InteractionClient;
use twilight_model::{
    application::interaction::{
        Interaction, InteractionData, InteractionType, application_command::CommandOptionValue,
    },
    gateway::presence::Status,
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::nu::ExecuteParams;

#[derive(Debug, Clone)]
pub struct DiscordHandler {
    pub execute_tx: tokio::sync::mpsc::Sender<ExecuteParams>,
    pub client: &'static twilight_http::Client,
    pub interaction_client: &'static InteractionClient<'static>,
    pub http_client: reqwest::Client,
}

impl DiscordHandler {
    pub async fn handle_interaction(
        &self,
        interaction: Json<Interaction>,
    ) -> axum::response::Result<Json<InteractionResponse>> {
        match interaction.kind {
            InteractionType::Ping => self.handle_ping().await,
            InteractionType::ApplicationCommand => {
                self.handle_application_command(interaction).await
            }
            InteractionType::MessageComponent => self.handle_message_component().await,
            InteractionType::ModalSubmit => self.handle_modal_submit().await,
            InteractionType::ApplicationCommandAutocomplete | _ => {
                Err(StatusCode::UNPROCESSABLE_ENTITY.into())
            }
        }
    }

    async fn handle_ping(&self) -> axum::response::Result<Json<InteractionResponse>> {
        Ok(Json(InteractionResponse {
            kind: InteractionResponseType::Pong,
            data: None,
        }))
    }

    async fn handle_application_command(
        &self,
        interaction: Json<Interaction>,
    ) -> axum::response::Result<Json<InteractionResponse>> {
        let data = interaction.0.data.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "application command should have data",
            )
        })?;
        let InteractionData::ApplicationCommand(data) = data else {
            return Err((
                StatusCode::BAD_REQUEST,
                "received interaction type application command but but got something else",
            )
                .into());
        };

        let source = data
            .options
            .iter()
            .find(|option| option.name == "source")
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "source is required"))?;
        let CommandOptionValue::String(ref source) = source.value else {
            return Err((
                StatusCode::BAD_REQUEST,
                "source option is expected to be a string",
            )
                .into());
        };

        let file = match data.options.iter().find(|option| option.name == "file") {
            None => None,
            Some(file) => {
                let CommandOptionValue::Attachment(file) = file.value else {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "file option is expected to be a string",
                    )
                        .into());
                };
                let resolved = data.resolved.ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        "expected resolved, because file exists",
                    )
                })?;
                let file = resolved
                    .attachments
                    .get(&file)
                    .ok_or_else(|| (StatusCode::BAD_REQUEST, "file appeared in options"))?;

                let file = self
                    .http_client
                    .get(&file.proxy_url)
                    .send()
                    .await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "could not GET proxy url"))?;
                let file = file.bytes().await.map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "could not read file bytes",
                    )
                })?;
                Some(file)
            }
        };

        let req = tokio::sync::oneshot::channel();
        self.execute_tx
            .send(ExecuteParams {
                fname: "fname".to_string(),
                source: source.clone(),
                file,
                res_tx: req.0,
            })
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "sending execute params failed",
                )
            })?;
        let res = req.1.await.expect("receiving execute response failed");
        Ok(Json(InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                InteractionResponseDataBuilder::new()
                    .content(format!("```ansi\n{res}\n```"))
                    .build(),
            ),
        }))
    }

    async fn handle_message_component(&self) -> axum::response::Result<Json<InteractionResponse>> {
        todo!()
    }

    async fn handle_modal_submit(&self) -> axum::response::Result<Json<InteractionResponse>> {
        todo!()
    }
}
