#![recursion_limit = "256"]
mod emotes;
mod exit;
mod handler;
mod neocities;
mod skiplist;
mod uwave;

use crate::handler::Handler;
use async_tungstenite::async_std::{connect_async, ConnectStream};
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures::prelude::*;
use sled::Db;
use crate::uwave::HttpApi;

#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    pub api_url: String,
    pub socket_url: String,
    pub email: String,
    pub password: String,
}

pub struct SekshiBot {
    database: Db,
    socket: WebSocketStream<ConnectStream>,
    api_url: String,
    handlers: Vec<Box<dyn Handler + Send>>,
}

impl SekshiBot {
    pub async fn connect(options: ConnectionOptions) -> anyhow::Result<Self> {
        let url = |endpoint: &str| format!("{}/{}", options.api_url, endpoint);

        log::info!("signing in...");
        let login = ureq::post(&url("auth/login"))
            .send_json(serde_json::json!({
                "email": options.email,
                "password": options.password,
            }))
            .into_json()?;

        let jwt = if let Some(jwt) = login["meta"]["jwt"].as_str() {
            jwt.to_string()
        } else {
            anyhow::bail!("no jwt found")
        };

        log::info!("loading state...");
        let now = ureq::get(&url("now"))
            .set("Authorization", &format!("JWT {}", jwt))
            .call();
        if !now.ok() {
            anyhow::bail!("could not fetch /now");
        }

        let now = now.into_json()?;
        let socket_token = match &now["socketToken"] {
            serde_json::Value::Null => None,
            serde_json::Value::String(token) => Some(token),
            _ => anyhow::bail!("unexpected socket token type"),
        };

        log::info!("connecting to {}...", options.socket_url);
        let (mut socket, _response) = connect_async(options.socket_url).await?;
        let database = sled::Config::default()
            .flush_every_ms(Some(1000))
            .path("sekshi.db")
            .open()?;

        if let Some(token) = socket_token {
            socket.send(Message::Text(token.to_string())).await?;
        }

        let mut bot = Self {
            database,
            socket,
            api_url: options.api_url,
            handlers: vec![],
        };

        let emotes = emotes::Emotes::new(&mut bot)?;
        bot.add_handler(emotes);
        bot.add_handler(exit::Exit);
        let skiplist = skiplist::SkipList::new(&mut bot, &now)?;
        bot.add_handler(skiplist);

        Ok(bot)
    }

    pub fn add_handler(&mut self, handler: impl Handler + Send + 'static) {
        self.handlers.push(Box::new(handler));
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (api_sender, api_receiver) = async_channel::bounded(10);
        let (received_message_sender, received_message_receiver) = async_channel::bounded(10);

        let mut api_receiver = api_receiver.fuse();
        let mut received_message_receiver = received_message_receiver.fuse();

        let mut socket = self.socket.fuse();
        let mut handlers = self.handlers;
        let http_api = HttpApi::new(self.api_url);

        let socket_stream = async move {
            loop {
                futures::select!(
                    message = socket.try_next() => {
                        let message = match message {
                            Ok(Some(Message::Text(message))) => {
                                if message == "-" {
                                    continue;
                                }
                                message
                            }
                            Ok(Some(_)) => continue,
                            Ok(None) => {
                                log::info!("connection ended");
                                break
                            },
                            Err(async_tungstenite::tungstenite::Error::ConnectionClosed) => {
                                log::info!("connection closed");
                                break
                            }
                            Err(err) => {
                                todo!("handle error {:?}", err)
                            }
                        };

                        let message: handler::Message = serde_json::from_str(&message).unwrap();

                        if let Some(message_type) = message.into_message_type() {
                            let _ = received_message_sender.send(message_type).await;
                        }
                    },
                    message = api_receiver.next() => match message {
                        Some(handler::ApiMessage::SendChat(message)) => {
                            socket.send(Message::Text(serde_json::json!({
                                "command": "sendChat",
                                "data": message,
                            }).to_string())).await?;
                        }
                        Some(handler::ApiMessage::Exit) | None => {
                            socket.close().await?;
                            break
                        }
                    }
                );
            }

            anyhow::Result::<()>::Ok(())
        };

        let handle_messages = async move {
            while let Some(message) = received_message_receiver.next().await {
                log::info!("handling message {:?}", message);
                let api = handler::Api::new(api_sender.clone(), http_api.clone());
                for handler in handlers.iter_mut() {
                    match handler.handle(api.clone(), &message).await {
                        Ok(..) => (),
                        Err(err) => {
                            api.send_message(format_args!("Could not handle message: {}", err))
                                .await;
                        }
                    }
                }
            }
        };

        let (socket_stream, _) = futures::join!(socket_stream, handle_messages,);
        let _ = socket_stream?;

        Ok(())
    }
}
