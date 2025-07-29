use anyhow::{Context, bail, ensure};
use std::sync::{Arc, atomic::AtomicBool};
use tracing::{error, instrument, warn};
use twilight_http::{Client, client::InteractionClient};
use twilight_model::{
    application::{
        command::CommandType,
        interaction::{
            Interaction, InteractionData, InteractionType, application_command::CommandData,
        },
    },
    channel::message::embed::EmbedField,
};
use twilight_util::builder::{
    command::{AttachmentBuilder, CommandBuilder, StringBuilder},
    embed::EmbedBuilder,
};
use itertools::Itertools;

use crate::{error_and_bail, executor::ExecuteParams, interaction::render::TerminalRenderer};

mod render;
mod cmd {
    pub mod delete_response;
    pub mod nu;
}

#[derive(Debug)]
pub struct InteractionHandler {
    discord_token: String,
    interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,
    wasm_ready: Arc<AtomicBool>,
    execute_tx: tokio::sync::mpsc::Sender<ExecuteParams>,
    http_client: reqwest::Client,
    terminal_renderer: TerminalRenderer,
}

impl InteractionHandler {
    pub fn new(
        discord_token: String,
        interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,
        wasm_ready: Arc<AtomicBool>,
        execute_tx: tokio::sync::mpsc::Sender<ExecuteParams>,
    ) -> Self {
        InteractionHandler {
            discord_token,
            interaction_rx,
            wasm_ready,
            execute_tx,
            http_client: reqwest::Client::new(),
            terminal_renderer: TerminalRenderer::new(),
        }
    }

    pub fn reply_ephemeral(interaction: &Interaction) -> bool {
        if let Some(InteractionData::ApplicationCommand(data)) = interaction.data.as_ref() {
            return data.kind != CommandType::ChatInput;
        }

        true
    }

    #[instrument(name = "interaction", skip_all)]
    pub async fn run(mut self) -> anyhow::Result<crate::Never> {
        let client = Client::new(std::mem::take(&mut self.discord_token));
        let interaction_client = client.interaction(crate::APPLICATION_ID);

        InteractionHandler::register_commands(&interaction_client)
            .await
            .context("Failed to register interaction commands")?;

        while let Some(interaction) = self.interaction_rx.recv().await {
            if let Err(err) = self
                .handle_interaction(interaction, &client, &interaction_client)
                .await
            {
                error!("{}", err.chain().join(": "));
            }
        }

        error_and_bail!("Interaction Handler stopped");
    }

    async fn handle_interaction(
        &mut self,
        mut interaction: Interaction,
        client: &Client,
        interaction_client: &InteractionClient<'_>,
    ) -> anyhow::Result<()> {
        match interaction.data.take() {
            Some(InteractionData::ApplicationCommand(data)) => {
                self.handle_application_command(interaction, client, interaction_client, *data)
                    .await
            }
            data => bail!("Unexpected interaction data: {data:?}"),
        }
    }

    async fn handle_application_command(
        &mut self,
        interaction: Interaction,
        client: &Client,
        interaction_client: &InteractionClient<'_>,
        data: CommandData,
    ) -> anyhow::Result<()> {
        ensure!(interaction.kind == InteractionType::ApplicationCommand);

        match data.name.as_str() {
            cmd::nu::COMMAND_NAME => {
                self.handle_nu_command(interaction, interaction_client, data)
                    .await
            }
            cmd::delete_response::COMMAND_NAME => {
                self.handle_delete_response_command(interaction, client, interaction_client, data)
                    .await
            }
            _ => todo!(),
        }
    }

    async fn register_commands(interaction_client: &InteractionClient<'_>) -> anyhow::Result<()> {
        let nu_command = CommandBuilder::new(
            cmd::nu::COMMAND_NAME,
            "Execute a nushell pipeline",
            CommandType::ChatInput,
        )
        .option(StringBuilder::new("source", "pipeline source code").required(true))
        .option(AttachmentBuilder::new("file", "input file"))
        .validate()?
        .build();
        let delete_command =
            CommandBuilder::new(cmd::delete_response::COMMAND_NAME, "", CommandType::Message)
                .validate()?
                .build();
        interaction_client
            .set_global_commands(&[nu_command, delete_command])
            .await?;
        Ok(())
    }

    async fn report(
        &self,
        client: &InteractionClient<'_>,
        token: impl AsRef<str>,
        title: impl AsRef<str>,
        description: impl Into<String>,
        color: u32,
        field: impl Into<Option<(String, anyhow::Error)>>,
    ) -> anyhow::Result<()> {
        let mut embed = EmbedBuilder::new()
            .title(title.as_ref())
            .description(description)
            .color(color);

        let field = field.into();
        if let Some((name, err)) = &field {
            embed = embed.field(EmbedField {
                inline: false,
                name: name.to_string(),
                value: Self::fmt_anyhow_error(err),
            });
        }

        match embed.validate() {
            Ok(embed) => {
                client
                    .update_response(token.as_ref())
                    .embeds(Some(&[embed.build()]))
                    .await?
            }
            Err(err) => {
                warn!("{err:?}");
                if let Some((_, err)) = field {
                    warn!("{err:?}");
                }

                client
                    .update_response(token.as_ref())
                    .content(Some(&format!(
                        "**Something went wrong.**\n-# Report that error to <@{}> if it occurs again.",
                        crate::SUPPORT_USER_ID
                    )))
                    .await?
            }
        };

        Ok(())
    }

    fn fmt_anyhow_error(err: &anyhow::Error) -> String {
        let formatted = format!("{err:?}");
        formatted
            .split("Stack backtrace")
            .next()
            .expect("is first")
            .to_owned()
    }
}
