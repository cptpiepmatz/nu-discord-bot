use twilight_model::application::interaction::Interaction;

pub struct InteractionHandler {
    interaction_rx: tokio::sync::mpsc::Receiver<Interaction>,
}

impl InteractionHandler {
    pub fn new(discord_token: String) -> Self {
        todo!()
    }

    pub async fn run(self) -> anyhow::Result<crate::Never> {
        panic!("Interaction Handler stopped.");
    }
}
