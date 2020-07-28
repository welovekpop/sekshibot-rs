use hreq::prelude::*;
use hreq::Agent;
use nanofd::FormData;
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

pub async fn publish(page_name: &str, content: &str) -> Result<String, PublishError> {
    let username = std::env::var("NEOCITIES_USERNAME").map_err(|_| PublishError::MissingAuth)?;
    let password = std::env::var("NEOCITIES_PASSWORD").map_err(|_| PublishError::MissingAuth)?;

    let mut agent = Agent::new();

    let mut form_data = FormData::new(Cursor::new(vec![]));
    let content_type = form_data.content_type();
    form_data.append_file(page_name, "text/html", &mut content.as_bytes())?;
    let data = form_data.end()?;

    let request = hreq::http::Request::post("https://neocities.org/api/upload")
        .header(
            "authorization",
            format!(
                "Basic {}",
                base64::encode(format!("{}:{}", username, password))
            ),
        )
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
