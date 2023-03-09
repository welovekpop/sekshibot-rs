use serde::Deserialize;
use sha1_smol::Sha1;
use std::io::Cursor;
use thiserror::Error;
use ureq::AgentBuilder;
use yolofd::FormData;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("missing neocities username/password")]
    MissingAuth,
    #[error("http error")]
    HttpError(#[from] Box<ureq::Error>),
    #[error("{0}")]
    NeocitiesError(String),
    #[error(transparent)]
    JsonError(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    path: String,
    sha1_hash: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    files: Vec<FileEntry>,
}

/// Returns the URL to the page.
pub fn publish(page_name: &str, content: &str) -> Result<String, PublishError> {
    let username = std::env::var("NEOCITIES_USERNAME").map_err(|_| PublishError::MissingAuth)?;
    let password = std::env::var("NEOCITIES_PASSWORD").map_err(|_| PublishError::MissingAuth)?;
    let authorization = format!(
        "Basic {}",
        base64::encode(format!("{username}:{password}"))
    );

    let client = AgentBuilder::new().build();

    let ListResponse { files, .. } = client
        .get("https://neocities.org/api/list")
        .set("authorization", &authorization)
        .call()
        .map_err(Box::new)?
        .into_json()?;
    if let Some(existing_page) = files.into_iter().find(|file| file.path == page_name) {
        let digest = Sha1::from(content).digest();

        if existing_page.sha1_hash == digest.to_string() {
            log::info!("not reuploading page {page_name}");
            return Ok(format!("https://{username}.neocities.org/{page_name}"));
        }
    }

    log::info!("uploading page {page_name}");
    let mut form_data = FormData::new(Cursor::new(vec![]));
    let content_type = form_data.content_type();
    form_data.append_file(page_name, "text/html", &mut content.as_bytes())?;
    let data = form_data.end()?;

    let json: serde_json::Value = client
        .post("https://neocities.org/api/upload")
        .set("authorization", &authorization)
        .set("content-type", &content_type)
        .send_bytes(&data.into_inner())
        .map_err(Box::new)?
        .into_json()?;

    if json["result"].as_str() == Some("error") {
        return Err(PublishError::NeocitiesError(
            json["message"].as_str().unwrap().to_string(),
        ));
    }

    Ok(format!("https://{username}.neocities.org/{page_name}"))
}
