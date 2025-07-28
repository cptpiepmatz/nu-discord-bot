use anyhow::{Context, bail, ensure};
use image::codecs::png::PngEncoder;
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tracing::{debug, error, instrument, warn};
use twilight_http::{Client, client::InteractionClient};
use twilight_model::{
    application::{
        command::CommandType,
        interaction::{
            Interaction, InteractionData, InteractionType, application_command::CommandOptionValue,
        },
    },
    channel::message::embed::EmbedField,
    http::attachment::Attachment,
};
use twilight_util::builder::{
    command::{AttachmentBuilder, CommandBuilder, StringBuilder},
    embed::EmbedBuilder,
};

use crate::{
    error_and_bail,
    executor::{ExecuteParams, ExecuteParamsFile, ExecuteResult},
    interaction::render::TerminalRenderer,
};

mod render;

#[derive(Debug)]
pub struct InteractionHandler {
    discord_token: String,
    interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,
    interaction_tokens: HashMap<String, String>,
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
            interaction_tokens: HashMap::new(),
            wasm_ready,
            execute_tx,
            http_client: reqwest::Client::new(),
            terminal_renderer: TerminalRenderer::new(),
        }
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
                .handle_interaction(interaction, &interaction_client)
                .await
            {
                error!("{err}");
            }
        }

        error_and_bail!("Interaction Handler stopped");
    }

    async fn handle_interaction(
        &mut self,
        interaction: Interaction,
        interaction_client: &InteractionClient<'_>,
    ) -> anyhow::Result<()> {
        debug!(
            "Got request from {}",
            interaction
                .author()
                .as_ref()
                .map(|user| user.name.as_str())
                .unwrap_or("unknown")
        );

        if !self.wasm_ready.load(Ordering::Relaxed) {
            self.report(
                &interaction_client,
                &interaction.token,
                "⚠️ Nu Executor Not Ready Yet",
                "The WASM runtime did not fully boot up yet.\nWait a bit and try again later.",
                crate::CONSTANTS.colors.yellow as u32,
                None,
            )
            .await
            .context("Failed to report Nu Executor Not Ready")?;
            return Ok(());
        }

        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let execute_params = match self.extract_execute_params(&interaction, result_tx).await {
            Ok(params) => params,
            Err(err) => {
                self.report(
                    &interaction_client,
                    &interaction.token,
                    "⚠️ Invalid Interaction Options",
                    format!(
                        "Discord sent invalid interaction options.\nReport this to <@{}>.",
                        crate::SUPPORT_USER_ID
                    ),
                    crate::CONSTANTS.colors.red as u32,
                    (
                        std::any::type_name_of_val(err.root_cause()).to_string(),
                        err,
                    ),
                )
                .await
                .context("Failed to report invalid interaction options")?;
                return Ok(());
            }
        };

        self.execute_tx
            .send(execute_params)
            .await
            .context("Failed to send execute parameters")?;

        match result_rx.await {
            Ok(Ok((res, elapsed))) => {
                debug!("Rendering result image");
                let image = self.terminal_renderer.render(&res);
                debug!("Encoding result image");
                let mut buf = Cursor::new(Vec::new());
                let encoder = PngEncoder::new(&mut buf);
                image
                    .write_with_encoder(encoder)
                    .expect("correctly allocated buf");
                debug!("Trying to respond in Discord");
                interaction_client
                    .update_response(&interaction.token)
                    .attachments(&[Attachment {
                        description: None,
                        file: buf.into_inner(),
                        filename: String::from("result.png"),
                        id: 0,
                    }])
                    .content(Some(&format!(
                        "-# took {}",
                        humantime::format_duration(elapsed)
                    )))
                    .await
                    .context("Failed to update response with result")?;
            }
            Ok(Err(err)) => {
                self.report(
                    &interaction_client,
                    &interaction.token,
                    "⚠️ Error During Nu Execution",
                    "An error while executing pipeline occurred.",
                    crate::CONSTANTS.colors.yellow as u32,
                    (
                        std::any::type_name_of_val(err.root_cause()).to_string(),
                        err,
                    ),
                )
                .await
                .context("Failed to report error during Nu execution")?;
            }
            Err(err) => {
                self.report(
                        &interaction_client,
                        &interaction.token,
                        "⚠️ Error Receiving Results",
                        format!(
                            "An error occurred while receiving the pipeline result.\nReport this to <@{}>.",
                            crate::SUPPORT_USER_ID
                        ),
                        crate::CONSTANTS.colors.red as u32,
                        (std::any::type_name_of_val(&err).to_string(), anyhow::Error::new(err)),
                    )
                    .await
                    .context("Failed to report error receiving results")?;
            }
        };

        Ok(())
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

    async fn extract_execute_params(
        &self,
        interaction: &Interaction,
        result_tx: tokio::sync::oneshot::Sender<ExecuteResult>,
    ) -> anyhow::Result<ExecuteParams> {
        ensure!(interaction.kind == InteractionType::ApplicationCommand);

        let Some(InteractionData::ApplicationCommand(data)) = &interaction.data else {
            bail!("expected interaction data to be an application command");
        };

        ensure!(data.name == "nu");
        ensure!(data.kind == CommandType::ChatInput);

        let source = data
            .options
            .iter()
            .find(|option| option.name == "source")
            .map(|option| &option.value)
            .context("missing source option")?;
        let source = match source {
            CommandOptionValue::String(source) => source,
            kind => bail!("expected source to be a string, got {}", kind.kind().kind()),
        };

        let file = match data
            .options
            .iter()
            .find(|option| option.name == "file")
            .map(|option| &option.value)
        {
            None => None,
            Some(file) => {
                let id = match file {
                    CommandOptionValue::Attachment(id) => id,
                    kind => bail!(
                        "expected file to be an attachment, got {}",
                        kind.kind().kind()
                    ),
                };

                let Some(resolved) = &data.resolved else {
                    bail!(
                        "expected interaction to have resolve data, because it contains file attachment"
                    );
                };

                let Some(file) = resolved.attachments.get(id) else {
                    bail!("could not find file attachment in resolved data");
                };

                let file = self.http_client.get(&file.url).send().await?;
                let file = file.bytes().await?;
                Some(match String::from_utf8(file.to_vec()) {
                    Ok(text) => ExecuteParamsFile::Text(text),
                    Err(_) => ExecuteParamsFile::Bytes(file),
                })
            }
        };

        Ok(ExecuteParams {
            result_tx,
            fname: "something".into(),
            source: source.to_owned(),
            file,
        })
    }

    async fn report(
        &self,
        client: &InteractionClient<'_>,
        token: &str,
        title: &str,
        description: impl Into<String>,
        color: u32,
        field: impl Into<Option<(String, anyhow::Error)>>,
    ) -> anyhow::Result<()> {
        let mut embed = EmbedBuilder::new()
            .title(title)
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
                    .update_response(token)
                    .embeds(Some(&[embed.build()]))
                    .await?
            }
            Err(err) => {
                warn!("{err:?}");
                if let Some((_, err)) = field {
                    warn!("{err:?}");
                }

                client
                    .update_response(token)
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
