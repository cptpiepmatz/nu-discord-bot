use std::sync::{LazyLock, OnceLock};

use anyhow::{Context, anyhow};
use ed25519_dalek::VerifyingKey;
use handlers::DiscordHandler;
use middleware::VERIFY_KEY;
use shuttle_runtime::SecretStore;
use static_toml::static_toml;
use twilight_http::{Client, client::InteractionClient};
use twilight_model::{
    application::command::CommandType,
    id::{Id, marker::ApplicationMarker},
};
use twilight_util::builder::command::{AttachmentBuilder, CommandBuilder, StringBuilder};

mod handlers;
mod middleware;
mod nu;
mod routes;

static_toml! {
    const CONSTANTS = include_toml!("../Constants.toml");
}

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(CONSTANTS.discord.application_id as u64);
const PUBLIC_KEY: [u8; 32] =
    match const_hex::const_decode_to_array(CONSTANTS.discord.public_key.as_bytes()) {
        Ok(value) => value,
        Err(_) => panic!("invalid public key"),
    };

#[shuttle_runtime::main]
async fn axum(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
    VERIFY_KEY
        .set(VerifyingKey::from_bytes(&PUBLIC_KEY).context("invalid verifying key")?)
        .map_err(|_| anyhow!("verify key somehow already set"))?;

    let discord_token = secrets
        .get("DISCORD_TOKEN")
        .context("missing DISCORD_TOKEN in secrets")?;
    let client = Box::leak(Box::new(Client::new(discord_token)));
    let interaction_client = Box::leak(Box::new(client.interaction(APPLICATION_ID)));

    register_commands(interaction_client).await?;

    let req = tokio::sync::mpsc::channel(4);

    tokio::spawn(nu::run(req.1));

    let router = routes::create_router(DiscordHandler {
        execute_tx: req.0,
        client,
        interaction_client,
        http_client: reqwest::Client::new(),
    });
    Ok(router.into())
}

async fn register_commands(interaction_client: &InteractionClient<'_>) -> anyhow::Result<()> {
    let nu_command =
        CommandBuilder::new("nu", "Execute a nushell pipeline", CommandType::ChatInput)
            .option(StringBuilder::new("source", "pipeline source code").required(true))
            .option(AttachmentBuilder::new("file", "input file"))
            .validate()?
            .build();
    interaction_client
        .set_global_commands(&[nu_command])
        .await?;
    Ok(())
}
