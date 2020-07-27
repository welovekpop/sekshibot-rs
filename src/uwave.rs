use crate::handler::MediaWithOverrides;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

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
struct ResponseData<Data, Meta> {
    pub data: Data,
    pub links: Links,
    pub meta: Meta,
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
pub struct HistoryEntry {
    pub media: MediaWithOverrides,
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
pub struct HttpApi {
    api_url: String,
}

impl HttpApi {
    pub fn new(api_url: String) -> Self {
        Self { api_url }
    }

    fn url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.api_url, endpoint)
    }

    pub async fn history(&self, opts: HistoryOptions) -> Result<Vec<HistoryEntry>> {
        let mut req = ureq::get(&self.url("booth/history"));
        let response = async_std::task::spawn(async move {
            if let Some(id) = opts.media {
                req.query("filter[media]", &id);
            }

            req.call()
                .into_json_deserialize::<ResponseData<_, PageMeta>>()
        })
        .await?;

        Ok(dbg!(response).data)
    }

    pub async fn skip(&self, opts: SkipOptions) -> Result<()> {
        let mut req = ureq::post(&self.url("booth/skip"));
        let response = async_std::task::spawn(async move {
            req.send_json(json!({
                "reason": opts.reason.unwrap_or_default(),
                "userID": opts.user_id,
                "remove": opts.remove,
            }))
            .into_json_deserialize()
        })
        .await?;

        Ok(response)
    }
}
