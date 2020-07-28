use crate::api::uwave::{HistoryOptions, SkipOptions};
use crate::handler::{Api, Handler, MessageType};
use anyhow::Result;
use chrono::{Duration, Utc};

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

        if results.len() < 2 {
            return Ok(());
        }

        let time = results[1].played_at;
        let ago = Utc::now() - time;
        if ago < Duration::hours(1) {
            log::info!("skipping because this song was played {} ago", ago);
            api.http
                .skip(SkipOptions {
                    reason: Some("history".to_string()), // format!("This song was played {} ago", ago))
                    user_id: message.user_id.clone(),
                    remove: false,
                })
                .await?;
        }

        Ok(())
    }
}
