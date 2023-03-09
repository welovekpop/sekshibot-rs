use crate::handler::{Api, ChatCommand, Handler, MessageType};

#[derive(Debug, Default)]
pub struct Version;

impl Handler for Version {
    fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
        let message = match message {
            MessageType::ChatMessage(message) => message,
            _ => return Ok(()),
        };
        let ChatCommand { command, .. } = if let Some(c) = message.command() {
            c
        } else {
            return Ok(());
        };

        if command.as_str() == "version" {
            api.send_message(format_args!(
                "Running {} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }

        Ok(())
    }
}
