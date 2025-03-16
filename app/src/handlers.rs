use axum::{Json, http::StatusCode};
use twilight_model::{
    application::interaction::{Interaction, InteractionType},
    http::interaction::{InteractionResponse, InteractionResponseType},
};

pub async fn discord(
    interaction: Json<Interaction>,
) -> Result<Json<InteractionResponse>, StatusCode> {
    match interaction.kind {
        InteractionType::Ping => handle_ping().await,
        InteractionType::ApplicationCommand => handle_application_command().await,
        InteractionType::MessageComponent => handle_message_component().await,
        InteractionType::ModalSubmit => handle_modal_submit().await,
        InteractionType::ApplicationCommandAutocomplete | _ => {
            Err(StatusCode::UNPROCESSABLE_ENTITY)
        }
    }
}

async fn handle_ping() -> Result<Json<InteractionResponse>, StatusCode> {
    Ok(InteractionResponse {
        kind: InteractionResponseType::Pong,
        data: None,
    }
    .into())
}

async fn handle_application_command() -> Result<Json<InteractionResponse>, StatusCode> {
    todo!()
}

async fn handle_message_component() -> Result<Json<InteractionResponse>, StatusCode> {
    todo!()
}

async fn handle_modal_submit() -> Result<Json<InteractionResponse>, StatusCode> {
    todo!()
}
