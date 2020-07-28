#![recursion_limit = "256"]
mod emotes;
mod exit;
mod handler;
mod historyskip;
mod neocities;
mod skiplist;
mod uwave;

use crate::handler::Handler;
use crate::uwave::{HttpApi, UnauthorizedError};
use async_tungstenite::async_std::{connect_async, ConnectStream};
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures::prelude::*;
use hreq::prelude::*;
use hreq::Agent;
use sled::Db;

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
    api_auth: String,
    handlers: Vec<Box<dyn Handler + Send>>,
}

impl SekshiBot {
    pub async fn connect(options: ConnectionOptions) -> anyhow::Result<Self> {
        let url = |endpoint: &str| format!("{}/{}", options.api_url, endpoint);
        let mut agent = Agent::new();

        log::info!("signing in...");
        let login = {
            let req = Request::post(&url("auth/login")).with_json(&serde_json::json!({
                "email": options.email,
                "password": options.password,
            }))?;

            let response = agent.send(req).await?;
            response
                .into_body()
                .read_to_json::<serde_json::Value>()
                .await?
        };

        let jwt = if let Some(jwt) = login["meta"]["jwt"].as_str() {
            jwt.to_string()
        } else {
            anyhow::bail!("no jwt found")
        };
        let api_auth = format!("JWT {}", jwt);

        log::info!("loading state...");
        let now = {
            let req = Request::get(&url("now"))
                .header("Authorization", &api_auth)
                .with_body(())?;
            let response = agent.send(req).await?;
            response
                .into_body()
                .read_to_json::<serde_json::Value>()
                .await?
        };

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
            api_auth,
            handlers: vec![],
        };

        let emotes = emotes::Emotes::new(&mut bot)?;
        bot.add_handler(emotes);
        bot.add_handler(exit::Exit);
        let skiplist = skiplist::SkipList::new(&mut bot, &now)?;
        bot.add_handler(skiplist);
        bot.add_handler(historyskip::HistorySkip);

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
        let http_api = HttpApi::new(&self.api_url, &self.api_auth);

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
                            socket.send(Message::Text(serde_json::json!({ "command": "logout" }).to_string())).await?;
                            socket.close().await?;
                            break
                        }
                    }
                );
            }

            anyhow::Result::<()>::Ok(())
        };

        let handle_messages = async move {
            let mut retval = Ok(());

            while let Some(message) = received_message_receiver.next().await {
                log::info!("handling message {:?}", message);
                let api = handler::Api::new(api_sender.clone(), http_api.clone());
                for handler in handlers.iter_mut() {
                    match handler.handle(api.clone(), &message).await {
                        Ok(..) => (),
                        Err(err) => {
                            // Exit if we are no longer authenticated so the bot can be restarted
                            if err.is::<UnauthorizedError>() {
                                api.exit().await;
                                retval = Err(err);
                                break;
                            }

                            api.send_message(format_args!("Could not handle message: {}", err))
                                .await;
                        }
                    }
                }
            }

            retval
        };

        let (socket_stream, handler_result) = futures::join!(socket_stream, handle_messages);
        let _ = socket_stream?;
        let _ = handler_result?;

        Ok(())
    }
}
