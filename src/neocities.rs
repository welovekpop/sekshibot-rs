use std::io::Cursor;
use thiserror::Error;
use nanofd::FormData;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("missing neocities username/password")]
    MissingAuth,
    #[error("{0}")]
    SyntheticError(String),
    #[error("{0}")]
    NeocitiesError(String),
    #[error(transparent)]
    JsonError(#[from] std::io::Error),
}

pub async fn publish(page_name: &str, content: &str) -> Result<String, PublishError> {
    let username = std::env::var("NEOCITIES_USERNAME").map_err(|_| PublishError::MissingAuth)?;
    let password = std::env::var("NEOCITIES_PASSWORD").map_err(|_| PublishError::MissingAuth)?;

    let mut form_data = FormData::new(Cursor::new(vec![]));
    let content_type = form_data.content_type();
    form_data.append_file(page_name, "text/html", &mut content.as_bytes())?;
    let data = form_data.end()?;

    let response = ureq::post("https://neocities.org/api/upload")
        .auth(&username, &password)
        .set("content-type", &content_type)
        .send_bytes(&data.into_inner());

    if let Some(err) = response.synthetic_error() {
        return Err(PublishError::SyntheticError(err.body_text()));
    }

    let json = dbg!(response.into_json()?);
    if json["result"].as_str() == Some("error") {
        return Err(PublishError::NeocitiesError(
            json["message"].as_str().unwrap().to_string(),
        ));
    }

    Ok(format!("https://{}.neocities.org/{}", username, page_name))
}
