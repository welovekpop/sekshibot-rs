use crate::api::uwave::{HistoryOptions, SkipOptions};
use crate::handler::{Api, Handler, MessageType};
use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};

#[derive(Debug)]
pub struct HistorySkip;

#[async_trait::async_trait]
impl Handler for HistorySkip {
    async fn handle(&mut self, api: Api<'_>, message: &MessageType) -> Result<()> {
        let message = match message {
            MessageType::Advance(advance) => advance,
            _ => return Ok(()),
        };

        let results = api
            .http
            .history(HistoryOptions {
                media: Some(message.media.media.id.clone()),
                ..Default::default()
            })
            .await?;

        let recent_entry = results
            .into_iter()
            // Skip the currently playing entry.
            .skip(1)
            .find(|entry| entry.media.media.id == message.media.media.id);

        let recent_entry = match recent_entry {
            Some(entry) => entry,
            None => return Ok(()),
        };

        let time = recent_entry.played_at;
        let ago = Utc::now() - time;
        if ago < Duration::hours(1) {
            let human_time = HumanTime::from(ago).to_text_en(Accuracy::Rough, Tense::Past);
            log::info!("skipping because this song was played {}", human_time);

            api.send_message(format!("This song was played {}.", human_time)).await;
            api.http
                .skip(SkipOptions {
                    reason: Some("history".to_string()),
                    user_id: message.user_id.clone(),
                    remove: false,
                })
                .await?;
        }

        Ok(())
    }
}
