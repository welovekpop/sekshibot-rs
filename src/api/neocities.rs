use hreq::prelude::*;
use hreq::Agent;
use nanofd::FormData;
use serde::Deserialize;
use sha1::Sha1;
use std::io::Cursor;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("missing neocities username/password")]
    MissingAuth,
    #[error(transparent)]
    HttpError(#[from] hreq::http::Error),
    #[error(transparent)]
    HreqError(#[from] hreq::Error),
    #[error("{0}")]
    NeocitiesError(String),
    #[error(transparent)]
    JsonError(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    is_directory: bool,
    path: String,
    sha1_hash: String,
    size: u32,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    files: Vec<FileEntry>,
    result: String,
}

fn list(username: &str, password: &str) -> Request<hreq::Body> {
    let auth = base64::encode(format!("{}:{}", username, password));

    Request::get("https://neocities.org/api/list")
        .header("authorization", format!("Basic {}", auth))
        .with_body(())
        .unwrap()
}

/// Returns the URL to the page.
pub async fn publish(page_name: &str, content: &str) -> Result<String, PublishError> {
    let username = std::env::var("NEOCITIES_USERNAME").map_err(|_| PublishError::MissingAuth)?;
    let password = std::env::var("NEOCITIES_PASSWORD").map_err(|_| PublishError::MissingAuth)?;

    let mut agent = Agent::new();

    let response = agent.send(list(&username, &password)).await?;
    let ListResponse { files, .. } = response.into_body().read_to_json().await?;
    if let Some(existing_page) = files.into_iter().find(|file| file.path == page_name) {
        let digest = Sha1::from(content).digest();

        if existing_page.sha1_hash == digest.to_string() {
            log::info!("not reuploading page {}", page_name);
            return Ok(format!("https://{}.neocities.org/{}", username, page_name));
        }
    }

    log::info!("uploading page {}", page_name);
    let mut form_data = FormData::new(Cursor::new(vec![]));
    let content_type = form_data.content_type();
    form_data.append_file(page_name, "text/html", &mut content.as_bytes())?;
    let data = form_data.end()?;

    let auth = base64::encode(format!("{}:{}", username, password));
    let request = hreq::http::Request::post("https://neocities.org/api/upload")
        .header("authorization", format!("Basic {}", auth))
        .header("content-type", &content_type)
        .with_body(&data.into_inner())?;

    let response = agent.send(request).await?;

    let json: serde_json::Value = response.into_body().read_to_json().await?;
    if json["result"].as_str() == Some("error") {
        return Err(PublishError::NeocitiesError(
            json["message"].as_str().unwrap().to_string(),
        ));
    }

    Ok(format!("https://{}.neocities.org/{}", username, page_name))
}
