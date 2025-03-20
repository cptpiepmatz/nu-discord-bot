use anyhow::{Context, anyhow};
use shuttle_runtime::SecretStore;
use static_toml::static_toml;
use twilight_model::id::{
    Id,
    marker::{ApplicationMarker, UserMarker},
};

mod handlers;
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
        Ok(App {
            http: http::HttpHandler::new()?,
            interaction: interaction::InteractionHandler::new(discord_token),
            executor: executor::NuExecutor::new(),
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

// async fn axum(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
//     VERIFY_KEY
//         .set(VerifyingKey::from_bytes(&PUBLIC_KEY).context("invalid verifying key")?)
//         .map_err(|_| anyhow!("verify key somehow already set"))?;

//     let discord_token = secrets
//         .get("DISCORD_TOKEN")
//         .context("missing DISCORD_TOKEN in secrets")?;
//     let client = Box::leak(Box::new(Client::new(discord_token)));
//     let interaction_client = Box::leak(Box::new(client.interaction(APPLICATION_ID)));

//     register_commands(interaction_client).await?;

//     let req = tokio::sync::mpsc::channel(4);

//     tokio::spawn(nu::run(req.1));

//     let router = routes::create_router(DiscordHandler {
//         execute_tx: req.0,
//         client,
//         interaction_client,
//         http_client: reqwest::Client::new(),
//     });
//     Ok(router.into())
// }

// async fn register_commands(interaction_client: &InteractionClient<'_>) -> anyhow::Result<()> {
//     let nu_command =
//         CommandBuilder::new("nu", "Execute a nushell pipeline", CommandType::ChatInput)
//             .option(StringBuilder::new("source", "pipeline source code").required(true))
//             .option(AttachmentBuilder::new("file", "input file"))
//             .validate()?
//             .build();
//     interaction_client
//         .set_global_commands(&[nu_command])
//         .await?;
//     Ok(())
// }
