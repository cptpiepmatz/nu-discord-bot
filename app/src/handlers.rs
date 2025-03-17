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
    ) -> anyhow::Result<Response> {
        match interaction.kind {
            InteractionType::Ping => self.handle_ping().await,
            InteractionType::ApplicationCommand => {
                self.handle_application_command(interaction).await
            }
            InteractionType::MessageComponent => self.handle_message_component().await,
            InteractionType::ModalSubmit => self.handle_modal_submit().await,
            InteractionType::ApplicationCommandAutocomplete | _ => {
                Err(StatusCode::UNPROCESSABLE_ENTITY)
            }
        }
    }

    async fn handle_ping(&self) -> anyhow::Result<Response> {
        Ok(Json(InteractionResponse {
            kind: InteractionResponseType::Pong,
            data: None,
        })
        .into_response())
    }

    async fn handle_application_command(
        &self,
        interaction: Json<Interaction>,
    ) -> anyhow::Result<Response> {
        let data = interaction
            .0
            .data
            .ok_or_else(|| anyhow!("application command should have data"))?;
        let InteractionData::ApplicationCommand(data) = data else {
            return Err(anyhow!(
                "received interaction type application command but but got something else"
            ));
        };

        let source = data
            .options
            .iter()
            .find(|option| option.name == "source")
            .ok_or_else(|| anyhow!("source is required"))?;
        let CommandOptionValue::String(ref source) = source.value else {
            return Err(anyhow!("source option is expected to be a string"));
        };

        let file = match data.options.iter().find(|option| option.name == "file") {
            None => None,
            Some(file) => {
                let CommandOptionValue::Attachment(file) = file.value else {
                    return Err(anyhow!("file option is expected to be a string"));
                };
                let resolved = data
                    .resolved
                    .ok_or_else(|| anyhow!("expected resolved, because file exists"))?;
                let file = resolved
                    .attachments
                    .get(&file)
                    .ok_or_else(|| anyhow!("file appeared in options"))?;

                let file = self
                    .http_client
                    .get(&file.proxy_url)
                    .send()
                    .await
                    .context("could not GET proxy url")?;
                let file = file.bytes().await.context("could not read file bytes")?;
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
            .context("sending execute params failed")?;
        let res = req.1.await.expect("receiving execute response failed");
        Ok(Json(InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                InteractionResponseDataBuilder::new()
                    .content(format!("```ansi\n{res}\n```"))
                    .build(),
            ),
        })
        .into_response())
    }

    async fn handle_message_component(&self) -> anyhow::Result<Response> {
        todo!()
    }

    async fn handle_modal_submit(&self) -> anyhow::Result<Response> {
        todo!()
    }
}
