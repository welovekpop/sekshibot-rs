use crate::IntoAnyhow;
use serde::Deserialize;
use sha1_smol::Sha1;
use std::io::Cursor;
use surf::Request;
use thiserror::Error;
use yolofd::FormData;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("missing neocities username/password")]
    MissingAuth,
    #[error("http error")]
    SurfError(#[from] anyhow::Error),
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

fn list(username: &str, password: &str) -> Request {
    let auth = base64::encode(format!("{}:{}", username, password));

    surf::get("https://neocities.org/api/list")
        .header("authorization", format!("Basic {}", auth))
        .build()
}

/// Returns the URL to the page.
pub async fn publish(page_name: &str, content: &str) -> Result<String, PublishError> {
    let username = std::env::var("NEOCITIES_USERNAME").map_err(|_| PublishError::MissingAuth)?;
    let password = std::env::var("NEOCITIES_PASSWORD").map_err(|_| PublishError::MissingAuth)?;

    let client = surf::client();

    let mut response = client
        .send(list(&username, &password))
        .await
        .into_anyhow_error()?;
    let ListResponse { files, .. } = response.body_json().await.into_anyhow_error()?;
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
    let request = surf::post("https://neocities.org/api/upload")
        .header("authorization", format!("Basic {}", auth))
        .header("content-type", &content_type)
        .body(data.into_inner());

    let mut response = client.send(request).await.into_anyhow_error()?;

    let json: serde_json::Value = response.body_json().await.into_anyhow_error()?;
    if json["result"].as_str() == Some("error") {
        return Err(PublishError::NeocitiesError(
            json["message"].as_str().unwrap().to_string(),
        ));
    }

    Ok(format!("https://{}.neocities.org/{}", username, page_name))
}
