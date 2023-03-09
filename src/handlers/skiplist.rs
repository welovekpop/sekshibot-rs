use crate::api::uwave::SkipOptions;
use crate::handler::{AdvanceMessage, Api, ChatCommand, ChatMessage, Handler, MessageType};
use rusqlite::{params, Connection, OptionalExtension as _};
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

fn get_media_from_now(now: &serde_json::Value) -> Option<Media> {
    let media = now.get("booth")?.get("media")?.get("media")?;
    Some(Media {
        source_type: media.get("sourceType")?.as_str()?.to_string(),
        source_id: media.get("sourceID")?.as_str()?.to_string(),
    })
}

#[derive(Debug)]
pub struct SkipList {
    current_media: Option<Media>,
}
impl SkipList {
    pub fn new(now: &serde_json::Value) -> Self {
        Self {
            current_media: get_media_from_now(now),
        }
    }

    fn add_skip_entry(
        &mut self,
        db: &Connection,
        media: Media,
        reason: &str,
    ) -> anyhow::Result<()> {
        log::info!("add entry {:?} {:?}", media.to_string(), reason);
        db.execute(
            "INSERT INTO skiplist (source_type, source_id, reason) VALUES (?, ?, ?)",
            params![media.source_type, media.source_id, reason,],
        )?;
        Ok(())
    }

    fn get_skip_entry(
        &mut self,
        db: &Connection,
        media: &Media,
    ) -> anyhow::Result<Option<SkipEntry>> {
        log::info!("check entry {:?}", media.to_string());
        let reason = db
            .query_row(
                "SELECT reason FROM skiplist WHERE source_type = ? AND source_id = ?",
                [&media.source_type, &media.source_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(reason.map(|reason| SkipEntry { reason }))
    }

    fn process_skip(&mut self, api: Api, args: &[String], do_skip: bool) -> anyhow::Result<()> {
        match args {
            [media, reason] => {
                self.add_skip_entry(&api.connection(), media.parse()?, reason)?;
            }
            [reason] => {
                if let Some(media) = self.current_media.clone() {
                    self.add_skip_entry(&api.connection(), media, reason)?;
                } else {
                    api.send_message("usage: !skiplist <media> <reason>");
                    return Ok(());
                }
            }
            _ => {
                api.send_message("usage: !skiplist [media] <reason>");
                return Ok(());
            }
        }

        if do_skip {
            api.http.skip(SkipOptions::default())?;
        }

        Ok(())
    }

    fn handle_chat_message(&mut self, api: Api, message: &ChatMessage) -> anyhow::Result<()> {
        let ChatCommand { command, arguments } = match message.command() {
            Some(c) => c,
            None => return Ok(()),
        };

        match command.as_str() {
            "skiplist" | "blacklist" => {
                match arguments.get(1).cloned().as_deref() {
                    Some("add") => {
                        self.process_skip(api, &arguments[2..], false)?;
                    }
                    Some("skip") => {
                        self.process_skip(api, &arguments[2..], true)?;
                    }
                    Some(_) => {
                        self.process_skip(api, &arguments[1..], false)?;
                    }
                    None => {
                        api.send_message("usage: !skiplist [media] <reason>");
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn handle_advance(&mut self, api: Api, message: &AdvanceMessage) -> anyhow::Result<()> {
        let media = Media {
            source_type: message.media.media.source_type.clone(),
            source_id: message.media.media.source_id.clone(),
        };

        if let Some(entry) = self.get_skip_entry(&api.connection(), &media)? {
            api.http.skip(SkipOptions {
                user_id: message.user_id.clone(),
                reason: Some(format!(
                    "This track is on the autoskip list: {}",
                    entry.reason
                )),
                remove: false,
            })?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Handler for SkipList {
    fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
        match message {
            MessageType::ChatMessage(message) => self.handle_chat_message(api, message),
            MessageType::Advance(message) => self.handle_advance(api, message),
            _ => Ok(()),
        }
    }
}
