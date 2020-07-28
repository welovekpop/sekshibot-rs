use anyhow::Result;
use chrono::{DateTime, Utc};
use hreq::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize)]
pub struct BaseMedia {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    #[serde(rename = "sourceID")]
    pub source_id: String,
    pub artist: String,
    pub title: String,
    pub duration: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaWithOverrides<T> {
    pub media: T,
    pub artist: String,
    pub title: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Links {
    #[serde(rename = "self")]
    pub self_: String,
    pub next: Option<String>,
    pub prev: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PageMeta {
    pub offset: u32,
    #[serde(rename = "pageSize")]
    pub page_size: u32,
    pub results: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct ResponseData<Data, Meta, Included> {
    pub data: Data,
    pub links: Links,
    pub meta: Meta,
    pub included: Included,
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 25,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistoryOptions {
    pub media: Option<String>,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Default)]
pub struct SkipOptions {
    pub user_id: String,
    pub reason: Option<String>,
    pub remove: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryEntry<TMedia> {
    pub media: MediaWithOverrides<TMedia>,
    upvotes: Vec<String>,
    downvotes: Vec<String>,
    favorites: Vec<String>,
    #[serde(rename = "_id")]
    pub history_id: String,
    #[serde(rename = "user")]
    pub user_id: String,
    #[serde(rename = "playedAt")]
    pub played_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HttpApi<'s> {
    api_url: &'s str,
    auth: &'s str,
}

impl<'s> HttpApi<'s> {
    pub fn new(api_url: &'s str, auth: &'s str) -> Self {
        Self { api_url, auth }
    }

    fn url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.api_url, endpoint)
    }

    pub async fn history(&self, opts: HistoryOptions) -> Result<Vec<HistoryEntry<BaseMedia>>> {
        let mut req = Request::get(&self.url("booth/history"));
        if let Some(id) = opts.media {
            req = req.query("filter[media]", &id);
        }

        #[derive(Debug, Deserialize)]
        struct IncludeHistory {
            media: Vec<BaseMedia>,
        }

        type ResponseShape = ResponseData<Vec<HistoryEntry<String>>, PageMeta, IncludeHistory>;

        let response = req
            .call()
            .await?
            .into_body()
            .read_to_json::<ResponseShape>()
            .await?;

        let ResponseData { data, included, .. } = response;

        let entries = data
            .into_iter()
            .map(|entry| HistoryEntry {
                media: MediaWithOverrides {
                    media: included
                        .media
                        .iter()
                        .find(|media| media.id == entry.media.media)
                        .unwrap()
                        .clone(),
                    artist: entry.media.artist,
                    title: entry.media.title,
                    start: entry.media.start,
                    end: entry.media.end,
                },
                upvotes: entry.upvotes,
                downvotes: entry.downvotes,
                favorites: entry.favorites,
                history_id: entry.history_id,
                user_id: entry.user_id,
                played_at: entry.played_at,
            })
            .collect();

        Ok(entries)
    }

    pub async fn skip(&self, opts: SkipOptions) -> Result<()> {
        let response = Request::post(&self.url("booth/skip"))
            .header("Authorization", self.auth)
            .send_json(&json!({
                "reason": opts.reason.unwrap_or_default(),
                "userID": opts.user_id,
                "remove": opts.remove,
            }))
            .await?;

        let json: serde_json::Value = response.into_body().read_to_json().await?;

        dbg!(json);

        Ok(())
    }
}
