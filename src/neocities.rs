use self::formdata::FormData;
use std::io::Cursor;
use thiserror::Error;

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
    form_data.write_file(page_name, "text/html", &mut content.as_bytes())?;
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

mod formdata {
    use std::io::{Read, Write};

    #[derive(Debug)]
    pub struct FormData<W>
    where
        W: Write,
    {
        writer: W,
        boundary: String,
    }

    impl<W> FormData<W>
    where
        W: Write,
    {
        pub fn new(writer: W) -> Self {
            Self {
                writer,
                boundary: format!("--------------------------{}", "aaaaaaaaaaaaaaaaaaaaaaaa"),
            }
        }

        pub fn content_type(&self) -> String {
            format!("multipart/form-data; boundary={}", self.boundary)
        }

        pub fn write_field(&mut self, name: &str, data: &str) -> std::io::Result<()> {
            write!(
                &mut self.writer,
                "--{}\r\nContent-Disposition: form-data; name={:?}\r\n\r\n{}\r\n",
                self.boundary, name, data
            )?;
            Ok(())
        }

        pub fn write_file(
            &mut self,
            name: &str,
            mime_type: &str,
            data: &mut impl Read,
        ) -> std::io::Result<()> {
            write!(
                &mut self.writer,
                "--{}\r\nContent-Disposition: form-data; name={:?}; filename={:?}\r\nContent-Type: {}\r\n\r\n",
                self.boundary, name, name, mime_type,
            )?;
            std::io::copy(data, &mut self.writer)?;
            write!(&mut self.writer, "\r\n")?;
            Ok(())
        }

        pub fn end(mut self) -> std::io::Result<W> {
            write!(&mut self.writer, "--{}--\r\n", self.boundary)?;
            Ok(self.writer)
        }

        pub fn into_inner(self) -> W {
            self.writer
        }
    }
}
