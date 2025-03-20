use std::sync::{Arc, atomic::AtomicBool};

use anyhow::Context;
use shuttle_runtime::SecretStore;
use static_toml::static_toml;
use twilight_model::id::{
    Id,
    marker::{ApplicationMarker, UserMarker},
};

mod nu;

mod executor;
mod http;
mod interaction;

static_toml! {
    const CONSTANTS = include_toml!("../Constants.toml");
}

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(CONSTANTS.discord.application_id as u64);
const SUPPORT_USER_ID: Id<UserMarker> = Id::new(CONSTANTS.discord.support_user_id as u64);
const PUBLIC_KEY: [u8; 32] =
    match const_hex::const_decode_to_array(CONSTANTS.discord.public_key.as_bytes()) {
        Ok(value) => value,
        Err(_) => panic!("invalid public key"),
    };

struct App {
    http: http::HttpHandler,
    interaction: interaction::InteractionHandler,
    executor: executor::NuExecutor,
}

impl App {
    fn new(discord_token: String) -> anyhow::Result<Self> {
        let interaction_channel = tokio::sync::mpsc::channel(1);
        let wasm_ready = Arc::new(AtomicBool::from(false));
        let execute_channel = tokio::sync::mpsc::channel(1);

        Ok(App {
            http: http::HttpHandler::new(interaction_channel.0)?,
            interaction: interaction::InteractionHandler::new(
                discord_token,
                interaction_channel.1,
                wasm_ready.clone(),
                execute_channel.0,
            ),
            executor: executor::NuExecutor::new(wasm_ready, execute_channel.1),
        })
    }
}

#[shuttle_runtime::main]
async fn shuttle_main(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> Result<App, shuttle_runtime::Error> {
    let discord_token = secrets
        .get("DISCORD_TOKEN")
        .context("missing DISCORD_TOKEN in secrets")?;

    Ok(App::new(discord_token)?)
}

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for App {
    async fn bind(self, addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let http = tokio::spawn(self.http.bind(addr));
        let interaction = tokio::spawn(self.interaction.run());
        let executor = tokio::spawn(self.executor.run());

        Err(tokio::select! {
            Err(err) = http => anyhow::Error::new(err).context("HTTP task panicked or was cancelled"),
            Err(err) = interaction => anyhow::Error::new(err).context("Interaction task panicked or was cancelled"),
            Err(err) = executor => anyhow::Error::new(err).context("Executor task panicked or was cancelled"),
        }.into())
    }
}

enum Never {}
