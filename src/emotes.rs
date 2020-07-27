use crate::handler::{Api, ChatCommand, Handler, MessageType};
use crate::neocities;
use crate::SekshiBot;
use std::fmt::Write as _;

#[derive(Debug)]
pub struct Emotes {
    tree: sled::Tree,
}
impl Emotes {
    pub fn new(bot: &mut SekshiBot) -> anyhow::Result<Self> {
        Ok(Self {
            tree: bot.database.open_tree("emotes")?,
        })
    }

    fn render_emote_page(&self) -> anyhow::Result<String> {
        let mut trs = String::new();
        for pair in self.tree.iter() {
            let (name, url) = pair?;

            write!(
                &mut trs,
                r#"
                <tr class="stripe-dark">
                  <td class="pv2 ph3 name">{id}</td>
                  <td class="pv2 ph3">
                    <a href="{url}" title="{url}" class="link dim light-pink" target="_blank">
                      {truncatedUrl}
                    </a>
                  </td>
                </tr>
                "#,
                id = std::str::from_utf8(&name)?,
                url = std::str::from_utf8(&url)?,
                truncatedUrl = std::str::from_utf8(&url)?
            )?;
        }

        let body = format!(
            r#"
            <body class="bg-dark-gray near-white mh5 mv3">
              <table class="collapse" style="margin: auto">
                <thead><tr>
                  <th class="pv2 ph3 ttu">Name</th>
                  <th class="pv2 ph3 ttu">URL</th>
                </tr></thead>
                <tbody>{}</tbody>
              </table>
            </body>
        "#,
            trs
        );

        let html = html_index::new()
            .raw_body(&body)
            .inline_style(tachyons::TACHYONS)
            .inline_script(
                r#"
                onload = function () {
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
                }
           "#,
            );

        Ok(html.build())
    }
}

#[async_trait::async_trait]
impl Handler for Emotes {
    async fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
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
                match self.tree.get(emote_name)? {
                    Some(bytes) => {
                        let emote = String::from_utf8(bytes.as_ref().to_vec())?;
                        api.send_message(emote).await;
                        Ok(())
                    }
                    None => Ok(()),
                }
            }
            "addemote" => {
                let emote_name = &arguments[0];
                let emote_url = &arguments[1];
                self.tree.insert(emote_name, emote_url.as_bytes())?;
                log::info!("insert {} {}", emote_name, emote_url);
                Ok(())
            }
            "emotes" => {
                let page = self.render_emote_page()?;
                let url = neocities::publish("emotes.html", &page).await?;
                api.send_message(url).await;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
