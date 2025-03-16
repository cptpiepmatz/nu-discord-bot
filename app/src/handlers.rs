use axum::{Json, http::StatusCode, response::Response};
use twilight_model::application::interaction::{Interaction, InteractionType};

pub async fn discord(interaction: Json<Interaction>) -> Result<Response, StatusCode> {
    match interaction.kind {
        InteractionType::Ping => handle_ping().await,
        InteractionType::ApplicationCommand => handle_application_command().await,
        InteractionType::MessageComponent => handle_message_component().await,
        InteractionType::ApplicationCommandAutocomplete => {
            handle_application_command_autocomplete().await
        }
        InteractionType::ModalSubmit => handle_modal_submit().await,
        _ => Err(StatusCode::UNPROCESSABLE_ENTITY),
    }
}

async fn handle_ping() -> Result<Response, StatusCode> {
    todo!()
}

async fn handle_application_command() -> Result<Response, StatusCode> {
    todo!()
}

async fn handle_message_component() -> Result<Response, StatusCode> {
    todo!()
}

async fn handle_application_command_autocomplete() -> Result<Response, StatusCode> {
    Err(StatusCode::UNPROCESSABLE_ENTITY)
}

async fn handle_modal_submit() -> Result<Response, StatusCode> {
    todo!()
}
