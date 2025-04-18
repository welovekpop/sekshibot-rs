use crate::handler::{Api, ChatCommand, Handler, MessageType};

#[derive(Debug, Default)]
pub struct Exit;

impl Handler for Exit {
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

        if command.as_str() == "exit" {
            let is_admin = api
                .http
                .user(&message.user_id)?
                .is_some_and(|user| user.roles.iter().any(|role| role == "admin"));

            if is_admin {
                api.exit();
            }
        }

        Ok(())
    }
}
