use anyhow::{Context, ensure};
use twilight_http::{client::InteractionClient, Client};
use twilight_model::application::{
    command::CommandType,
    interaction::{Interaction, application_command::CommandData},
};

use crate::interaction::InteractionHandler;

pub const COMMAND_NAME: &str = "Delete Nushell Response";

impl InteractionHandler {
    pub async fn handle_delete_response_command(
        &mut self,
        interaction: Interaction,
        client: &Client,
        interaction_client: &InteractionClient<'_>,
        data: CommandData,
    ) -> anyhow::Result<()> {
        ensure!(data.name == COMMAND_NAME);
        ensure!(data.kind == CommandType::Message);

        // dbg!(&interaction, &data);

        let resolved = data
            .resolved
            .context("Got no resolved data for Message command")?;
        let (_, message) = resolved
            .messages
            .into_iter()
            .next()
            .context("Go no resolved message for Message command")?;

        if message.author.id != crate::APPLICATION_ID.cast() {
            return self.report(
                interaction_client,
                interaction.token,
                ":warning: Message not by this bot",
                format!("This message wasn't sent by <@{}>.\nYou cannot delete this.", crate::APPLICATION_ID),
                crate::CONSTANTS.colors.yellow as u32,
                None,
            ).await;
        }

        let Some(interaction_metadata) = message.interaction_metadata else {
            return self
                .report(
                    interaction_client,
                    interaction.token,
                    ":warning: Not a Nushell Response",
                    "This message doesn't seem to be a Nushell response.\nYou cannot delete this.",
                    crate::CONSTANTS.colors.yellow as u32,
                    None,
                )
                .await;
        };

        let target_author_id = interaction_metadata.user.id;
        let interaction_author_id = interaction.author_id().context("Got no author ID for interaction")?;

        if target_author_id == interaction_author_id {
                // FIXME: deleting message doesn't work, maybe via deleting interaction
                client.delete_message(message.channel_id, message.id).await.context("Could not delete message")?;
        }

        dbg!(interaction_metadata);

        Ok(())
    }
}
