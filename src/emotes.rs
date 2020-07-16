use crate::handler::{Api, ChatCommand, Handler, MessageType};
use crate::SekshiBot;

#[derive(Debug)]
pub struct Emotes {
    tree: sled::Tree,
}
impl Emotes {
    pub fn new(bot: &mut SekshiBot) -> anyhow::Result<Self> {
        Ok(Self {
            tree: bot.database.open_tree("emotes")?,
        })
    }
}

#[async_trait::async_trait]
impl Handler for Emotes {
    async fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
        let message = match message {
            MessageType::ChatMessage(message) => message,
            _ => return Ok(()),
        };

        let ChatCommand { command, arguments } = match message.command() {
            Some(c) => c,
            None => return Ok(()),
        };
        match command.as_str() {
            "e" | "emote" => {
                let emote_name = &arguments[0];
                match self.tree.get(emote_name)? {
                    Some(bytes) => {
                        let emote = String::from_utf8(bytes.as_ref().to_vec())?;
                        api.send_message(emote).await;
                        Ok(())
                    }
                    None => Ok(()),
                }
            }
            "addemote" => {
                let emote_name = &arguments[0];
                let emote_url = &arguments[1];
                self.tree.insert(emote_name, emote_url.as_bytes())?;
                log::info!("insert {} {}", emote_name, emote_url);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
