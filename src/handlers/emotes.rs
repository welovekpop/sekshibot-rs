use crate::api::neocities;
use crate::handler::{Api, ChatCommand, Handler, MessageType};
use crate::SekshiBot;
use indoc::indoc;
use rusqlite::{Connection, OptionalExtension as _};
use shorten_url::shorten;

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
        log::info!("insert {name} {url}");
        db.execute("INSERT INTO emotes (name, url) VALUES (?, ?)", [name, url])?;
        Ok(())
    }

    fn render_emote_page(&self, db: &Connection) -> anyhow::Result<String> {
        let mut stmt = db.prepare("SELECT name, url FROM emotes")?;
        let query = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let body = maud::html! {
            table {
                thead {
                    tr {
                        th { "Name" }
                        th { "URL" }
                    }
                }
                tbody {
                    @for pair in query {
                        @let (name, url) = pair?;
                        tr {
                            td { (name) }
                            td {
                                a href=(url) title=(url) target="_blank" { (shorten(&url, 50)) }
                            }
                        }
                    }
                }
            }
        };

        let html = maud::html! {
            (maud::DOCTYPE)
            html {
                head {
                    style {
                        (maud::PreEscaped(indoc! {r#"
                            body { margin: 1rem 4rem; background: #333; color: #f4f4f4; font-family: sans-serif; }
                            table { border-collapse: collapse; border-spacing: 0; margin: auto; }
                            tbody > tr:nth-child(2n+1) { background-color: #0000001a; }
                            th, td { padding: .5rem 1rem; }
                            th { text-transform: uppercase; }
                            a { text-decoration: none; color: #ffa3d7; }
                            a:hover { text-decoration: underline; }
                        "#}))
                    }
                }
                body {
                    (body)
                    script defer {
                        (indoc! {r#"
                            if (document.body.classList) onclick = function onclick (event) {
                              if (!event.target.classList.contains('name')) {
                                return
                              }
                              var s = window.getSelection()
                              var r = document.createRange()
                              r.selectNodeContents(event.target)
                              s.removeAllRanges()
                              s.addRange(r)
                            }
                        "#})
                    }
                }
            }
        };

        Ok(html.into_string())
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
                api.send_message(format_args!("{emote_name} added!"));
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
