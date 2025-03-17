use axum::{Json, http::StatusCode};
use twilight_model::{
    application::interaction::{Interaction, InteractionType},
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::nu::ExecuteParams;

#[derive(Debug, Clone)]
pub struct DiscordHandler {
    pub execute_tx: tokio::sync::mpsc::Sender<ExecuteParams>,
}

impl DiscordHandler {
    pub async fn handle_interaction(
        &self,
        interaction: Json<Interaction>,
    ) -> Result<Json<InteractionResponse>, StatusCode> {
        match interaction.kind {
            InteractionType::Ping => self.handle_ping().await,
            InteractionType::ApplicationCommand => self.handle_application_command().await,
            InteractionType::MessageComponent => self.handle_message_component().await,
            InteractionType::ModalSubmit => self.handle_modal_submit().await,
            InteractionType::ApplicationCommandAutocomplete | _ => {
                Err(StatusCode::UNPROCESSABLE_ENTITY)
            }
        }
    }

    async fn handle_ping(&self) -> Result<Json<InteractionResponse>, StatusCode> {
        Ok(InteractionResponse {
            kind: InteractionResponseType::Pong,
            data: None,
        }
        .into())
    }

    async fn handle_application_command(&self) -> Result<Json<InteractionResponse>, StatusCode> {
        let req = tokio::sync::oneshot::channel();
        self.execute_tx
            .send(ExecuteParams {
                fname: "fname".to_string(),
                source: "some source".to_string(),
                file: None,
                res_tx: req.0,
            })
            .await
            .expect("sending execute params failed");
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

    async fn handle_message_component(&self) -> Result<Json<InteractionResponse>, StatusCode> {
        todo!()
    }

    async fn handle_modal_submit(&self) -> Result<Json<InteractionResponse>, StatusCode> {
        todo!()
    }
}
