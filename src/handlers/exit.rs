use crate::handler::{Api, ChatCommand, Handler, MessageType};

#[derive(Debug, Default)]
pub struct Exit;

#[async_trait::async_trait]
impl Handler for Exit {
    async fn handle(&mut self, api: Api<'_>, message: &MessageType) -> anyhow::Result<()> {
        let message = match message {
            MessageType::ChatMessage(message) => message,
            _ => return Ok(()),
        };
        let ChatCommand { command, .. } = if let Some(c) = message.command() {
            c
        } else {
            return Ok(());
        };

        if command.as_str() == "exit" {
            api.exit().await;
        }

        Ok(())
    }
}
