use anyhow::{Context, bail, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::instrument;
use twilight_http::{Client, client::InteractionClient};
use twilight_model::{
    application::{
        command::CommandType,
        interaction::{
            Interaction, InteractionData, InteractionType, application_command::CommandOptionValue,
        },
    },
    channel::message::{MessageFlags, embed::EmbedField},
};
use twilight_util::builder::{
    command::{AttachmentBuilder, CommandBuilder, StringBuilder},
    embed::EmbedBuilder,
};

use crate::{
    error_and_bail,
    executor::{ExecuteParams, ExecuteParamsFile, ExecuteResult},
};

#[derive(Debug)]
pub struct InteractionHandler {
    discord_token: String,
    interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,
    wasm_ready: Arc<AtomicBool>,
    execute_tx: tokio::sync::mpsc::Sender<ExecuteParams>,
    http_client: reqwest::Client,
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
        }
    }

    #[instrument(name = "interaction", skip_all)]
    pub async fn run(mut self) -> anyhow::Result<crate::Never> {
        let client = Client::new(std::mem::take(&mut self.discord_token));
        let interaction_client = client.interaction(crate::APPLICATION_ID);

        InteractionHandler::register_commands(&interaction_client).await?;

        while let Some(interaction) = self.interaction_rx.recv().await {
            if !self.wasm_ready.load(Ordering::Relaxed) {
                self.report(
                    &interaction_client,
                    &interaction.token,
                    "⚠️ Nu Executor Not Ready Yet",
                    "The WASM runtime did not fully boot up yet.\nWait a bit and try again later.",
                    crate::CONSTANTS.colors.yellow as u32,
                    None,
                )
                .await?;
                continue;
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
                            err.to_string(),
                        ),
                    )
                    .await?;
                    continue;
                }
            };

            self.execute_tx
                .send(execute_params)
                .await
                .context("could not send execute params")?;

            match result_rx.await {
                Ok(Ok(res)) => {
                    let content = format!("```ansi\n{res}\n```");
                    match content.chars().count() {
                        ..=2000 => {
                            interaction_client
                                .update_response(&interaction.token)
                                .content(Some(&content))
                                .await?;
                        }
                        len => {
                            self.report(
                                &interaction_client,
                                &interaction.token,
                                "⚠️ Result Too Long",
                                format!("The result is {len} characters long — that's over Discord's 2000 character limit. Try adjusting your pipeline to make the output smaller."),
                                crate::CONSTANTS.colors.yellow as u32,
                                None
                            ).await?;
                        }
                    };
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
                            err.to_string(),
                        ),
                    )
                    .await?;
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
                        (std::any::type_name_of_val(&err).to_string(), err.to_string())
                    ).await?;
                }
            };
        }

        error_and_bail!("Interaction Handler stopped");
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
        field: impl Into<Option<(String, String)>>,
    ) -> anyhow::Result<()> {
        let mut embed = EmbedBuilder::new()
            .title(title)
            .description(description)
            .color(color);

        if let Some((name, value)) = field.into() {
            embed = embed.field(EmbedField {
                inline: false,
                name: name.to_string(),
                value: value.to_string(),
            });
        }

        client.delete_response(token).await?;

        client
            .create_followup(token)
            .flags(MessageFlags::EPHEMERAL)
            .embeds(&[embed.build()])
            .await?;

        Ok(())
    }
}
