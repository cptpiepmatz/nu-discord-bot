use anyhow::{Context, bail, ensure};
use image::codecs::png::PngEncoder;
use std::{io::Cursor, sync::atomic::Ordering};
use tracing::debug;
use twilight_http::client::InteractionClient;
use twilight_model::{
    application::{
        command::CommandType,
        interaction::{
            Interaction,
            application_command::{CommandData, CommandOptionValue},
        },
    },
    http::attachment::Attachment,
};

use crate::{
    executor::{ExecuteParams, ExecuteParamsFile, ExecuteResult},
    interaction::InteractionHandler,
};

pub const COMMAND_NAME: &str = "nu";

impl InteractionHandler {
    pub async fn handle_nu_command(
        &mut self,
        interaction: Interaction,
        interaction_client: &InteractionClient<'_>,
        data: CommandData,
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
        let execute_params = match self.extract_execute_params(data, result_tx).await {
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

    async fn extract_execute_params(
        &self,
        data: CommandData,
        result_tx: tokio::sync::oneshot::Sender<ExecuteResult>,
    ) -> anyhow::Result<ExecuteParams> {
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
}
