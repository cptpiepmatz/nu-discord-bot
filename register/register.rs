use static_toml::static_toml;
use twilight_http::Client;
use twilight_model::{
    application::command::CommandType,
    id::{Id, marker::ApplicationMarker},
};
use twilight_util::builder::command::{AttachmentBuilder, CommandBuilder, StringBuilder};

static_toml! {
    #[static_toml(auto_doc = false)]
    static SECRETS = include_toml!("../Secrets.toml");
    const CONSTANTS = include_toml!("../Constants.toml");
}

const APPLICATION_ID: Id<ApplicationMarker> = Id::new(CONSTANTS.discord.application_id as u64);

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let client = Client::new(SECRETS.discord.token.to_owned());
    let interaction_client = client.interaction(APPLICATION_ID);

    let nu_command =
        CommandBuilder::new("nu", "Execute a nushell pipeline", CommandType::ChatInput)
            .option(StringBuilder::new("source", "pipeline source code").required(true))
            .option(AttachmentBuilder::new("file", "input file"))
            .validate()
            .unwrap()
            .build();
    interaction_client
        .set_global_commands(&[nu_command])
        .await
        .unwrap();
}
