use crate::handler::{AdvanceMessage, Api, ChatCommand, ChatMessage, Handler, MessageType};
use crate::uwave::SkipOptions;
use crate::SekshiBot;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Media {
    source_type: String,
    source_id: String,
}

#[derive(Debug, thiserror::Error)]
#[error("failed to parse media ID. expected format: `sourcetype:id`")]
pub struct ParseMediaIDError;

impl FromStr for Media {
    type Err = ParseMediaIDError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(source_type), Some(source_id)) => Ok(Self {
                source_type: source_type.to_string(),
                source_id: source_id.to_string(),
            }),
            _ => Err(ParseMediaIDError),
        }
    }
}

impl Display for Media {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.source_type, self.source_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkipEntry {
    reason: String,
}

#[derive(Debug)]
pub struct SkipList {
    tree: sled::Tree,
    current_media: Option<Media>,
}
impl SkipList {
    pub fn new(bot: &mut SekshiBot, now: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            tree: bot.database.open_tree("skiplist")?,
            current_media: now
                .get("booth")
                .and_then(|booth| booth.get("media"))
                .and_then(|media| media.get("media"))
                .and_then(|media| {
                    Some(Media {
                        source_type: media.get("sourceType")?.as_str()?.to_string(),
                        source_id: media.get("sourceID")?.as_str()?.to_string(),
                    })
                }),
        })
    }

    fn add_skip_entry(&mut self, media: Media, reason: &str) -> Result<()> {
        let media = media.to_string();
        let entry = serde_json::to_vec(&SkipEntry {
            reason: reason.to_string(),
        })?;
        log::info!("add entry {:?} {:?}", media, entry);
        self.tree.insert(media, entry)?;
        Ok(())
    }

    fn get_skip_entry(&mut self, media: &Media) -> Result<Option<SkipEntry>> {
        let media = media.to_string();
        log::info!("check entry {:?}", media);
        if let Some(entry) = self.tree.get(media)? {
            let entry = serde_json::from_slice(&entry)?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn handle_chat_message(&mut self, api: Api, message: &ChatMessage) -> Result<()> {
        let ChatCommand { command, arguments } = match message.command() {
            Some(c) => c,
            None => return Ok(()),
        };

        match command.as_str() {
            "skiplist" | "blacklist" => {
                match arguments.as_slice() {
                    [media, reason] => {
                        self.add_skip_entry(media.parse()?, reason)?;
                    }
                    [reason] => {
                        if let Some(media) = self.current_media.clone() {
                            self.add_skip_entry(media, reason)?;
                        } else {
                            api.send_message("usage: !skiplist <media> <reason>").await;
                        }
                    }
                    _ => {
                        api.send_message("usage: !skiplist [media] <reason>").await;
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn handle_advance(&mut self, api: Api, message: &AdvanceMessage) -> Result<()> {
        let media = Media {
            source_type: message.media.media.source_type.clone(),
            source_id: message.media.media.source_id.clone(),
        };

        if let Some(entry) = self.get_skip_entry(&media)? {
            api.http
                .skip(SkipOptions {
                    user_id: message.user_id.clone(),
                    reason: Some(format!(
                        "This track is on the autoskip list: {}",
                        entry.reason
                    )),
                    remove: false,
                })
                .await?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl Handler for SkipList {
    async fn handle(&mut self, api: Api, message: &MessageType) -> Result<()> {
        match message {
            MessageType::ChatMessage(message) => self.handle_chat_message(api, message).await,
            MessageType::Advance(message) => self.handle_advance(api, message).await,
            _ => return Ok(()),
        }
    }
}
