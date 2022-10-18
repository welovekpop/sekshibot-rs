use crate::api::neocities;
use crate::handler::{Api, ChatCommand, Handler, MessageType};
use crate::SekshiBot;
use rusqlite::{Connection, OptionalExtension as _};
use shorten_url::shorten;
use std::fmt::Write as _;

#[derive(Debug)]
pub struct Emotes;
impl Emotes {
    pub fn new(_bot: &mut SekshiBot) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn get_emote(&self, db: &Connection, name: &str) -> anyhow::Result<Option<String>> {
        let url = db
            .query_row("SELECT url FROM emotes WHERE name = ?", [name], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(url)
    }

    fn insert_emote(&self, db: &Connection, name: &str, url: &str) -> anyhow::Result<()> {
        log::info!("insert {} {}", name, url);
        db.execute("INSERT INTO emotes (name, url) VALUES (?, ?)", [name, url])?;
        Ok(())
    }

    fn render_emote_page(&self, db: &Connection) -> anyhow::Result<String> {
        let mut stmt = db.prepare("SELECT name, url FROM emotes")?;
        let query = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut trs = String::new();
        for pair in query {
            let (name, url) = pair?;

            write!(
                &mut trs,
                r#"
                <tr>
                  <td class="name">{id}</td>
                  <td>
                    <a href="{url}" title="{url}" target="_blank">
                      {truncatedUrl}
                    </a>
                  </td>
                </tr>
                "#,
                id = html_escape::encode_text(&name),
                url = html_escape::encode_double_quoted_attribute(&url),
                truncatedUrl = html_escape::encode_text(&shorten(&url, 50))
            )?;
        }

        let body = format!(
            r#"
            <body>
              <table>
                <thead><tr>
                  <th>Name</th>
                  <th>URL</th>
                </tr></thead>
                <tbody>{}</tbody>
              </table>
              <script defer>
                if (document.body.classList) onclick = function onclick (event) {{
                  if (!event.target.classList.contains('name')) {{
                    return
                  }}
                  var s = window.getSelection()
                  var r = document.createRange()
                  r.selectNodeContents(event.target)
                  s.removeAllRanges()
                  s.addRange(r)
                }}
              </script>
            </body>
        "#,
            trs
        );

        let body = minify_html::minify(body.as_bytes(), &minify_html::Cfg {
            minify_css: true,
            minify_js: true,
            ..Default::default()
        });

        let html = html_index::new()
            .raw_body(std::str::from_utf8(&body)?)
            .inline_style(r#"
                body { margin: 1rem 4rem; background: #333; color: #f4f4f4; font-family: sans-serif; }
                table { border-collapse: collapse; border-spacing: 0; margin: auto; }
                tbody > tr:nth-child(2n+1) { background-color: #0000001a; }
                th, td { padding: .5rem 1rem; }
                th { text-transform: uppercase; }
                a { text-decoration: none; color: #ffa3d7; }
                a:hover { text-decoration: underline; }
            "#);

        Ok(html.build())
    }
}

impl Handler for Emotes {
    fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
        let message = match message {
            MessageType::ChatMessage(message) => message,
            _ => return Ok(()),
        };

        let ChatCommand { command, arguments } = match message.command() {
            Some(c) => c,
            None => return Ok(()),
        };
        match command.as_str() {
            "e" | "emote" => {
                let emote_name = &arguments[0];
                if let Some(url) = self.get_emote(&api.connection(), emote_name)? {
                    api.send_message(url);
                }
                Ok(())
            }
            "addemote" => {
                let emote_name = &arguments[0];
                let emote_url = &arguments[1];
                self.insert_emote(&api.connection(), emote_name, emote_url)?;
                api.send_message(format_args!("{} added!", emote_name));
                Ok(())
            }
            "emotes" => {
                let page = self.render_emote_page(&api.connection())?;
                let url = neocities::publish("emotes.html", &page)?;
                api.send_message(url);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
