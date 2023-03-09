use crate::api::uwave::{HistoryOptions, SkipOptions};
use crate::handler::{Api, Handler, MessageType};
use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};

#[derive(Debug, Default)]
pub struct HistorySkip {
    consecutive_skip_count: usize,
}

impl HistorySkip {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Handler for HistorySkip {
    fn handle(&mut self, api: Api, message: &MessageType) -> Result<()> {
        let message = match message {
            MessageType::Advance(advance) => advance,
            _ => return Ok(()),
        };

        let results = api.http.history(HistoryOptions {
            media: Some(message.media.media.id.clone()),
            ..Default::default()
        })?;

        let recent_entry = results
            .into_iter()
            // Ignore the currently playing entry.
            .skip(1)
            .find(|entry| entry.media.media.id == message.media.media.id);

        let Some(recent_entry) = recent_entry else {
            self.consecutive_skip_count = 0;
            return Ok(())
        };

        let time = recent_entry.played_at;
        let ago = Utc::now() - time;
        if ago < Duration::hours(1) {
            let human_time = HumanTime::from(ago).to_text_en(Accuracy::Rough, Tense::Past);
            log::info!("skipping because this song was played {}", human_time);

            self.consecutive_skip_count += 1;
            api.send_message(format!("This song was played {}.", human_time));
            api.http.skip(SkipOptions {
                reason: Some("history".to_string()),
                user_id: message.user_id.clone(),
                remove: self.consecutive_skip_count > 3,
            })?;
        } else {
            self.consecutive_skip_count = 0;
        }

        Ok(())
    }
}
