#![recursion_limit = "512"]
mod handler;
mod handlers;
mod migrations;
mod api {
    pub mod neocities;
    pub mod uwave;
}

use crate::api::uwave::HttpApi;
use crate::handler::Handler;
use r2d2_sqlite::SqliteConnectionManager;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;
use ureq::{Agent, AgentBuilder};

// Expose so the CLI can use a special exit code
pub use crate::api::uwave::UnauthorizedError;

type WebSocket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    pub api_url: String,
    pub socket_url: String,
    pub email: String,
    pub password: String,
}

pub struct SekshiBot {
    pool: r2d2::Pool<SqliteConnectionManager>,
    client: Agent,
    socket: WebSocket,
    api_url: String,
    api_auth: String,
    handlers: Vec<Box<dyn Handler + Send>>,
}

fn connect_ws(url: &str) -> anyhow::Result<WebSocket> {
    use native_tls::TlsConnector;
    use url::Url;
    let url = Url::parse(url)?;
    if url.scheme() == "wss" {
        let socket_addrs = url.socket_addrs(|| None)?.remove(0);
        let connector = TlsConnector::new()?;
        let stream = TcpStream::connect(socket_addrs)?;
        let tls_stream = connector.connect(url.host_str().unwrap(), stream.try_clone()?)?;
        let (socket, _response) = tungstenite::client(url, MaybeTlsStream::NativeTls(tls_stream))?;
        stream.set_nonblocking(true)?;

        Ok(socket)
    } else {
        let socket_addrs = url.socket_addrs(|| None)?.pop().unwrap();
        let stream = TcpStream::connect(socket_addrs)?;
        let (socket, _response) =
            tungstenite::client(url, MaybeTlsStream::Plain(stream.try_clone()?))?;
        stream.set_nonblocking(true)?;
        Ok(socket)
    }
}

impl SekshiBot {
    pub fn connect(options: ConnectionOptions) -> anyhow::Result<Self> {
        let url = |endpoint: &str| format!("{}/{}", options.api_url, endpoint);
        let client = AgentBuilder::new().build();

        log::info!("signing in...");
        let login = client
            .post(&url("auth/login"))
            .send_json(serde_json::json!({
                "email": options.email,
                "password": options.password,
            }))?
            .into_json::<serde_json::Value>()?;

        let jwt = if let Some(jwt) = login["meta"]["jwt"].as_str() {
            jwt.to_string()
        } else {
            anyhow::bail!("no jwt found")
        };
        let api_auth = format!("JWT {}", jwt);

        log::info!("loading state...");
        let now = client
            .get(&url("now"))
            .set("Authorization", &api_auth)
            .call()?
            .into_json::<serde_json::Value>()?;

        let socket_token = match &now["socketToken"] {
            serde_json::Value::Null => None,
            serde_json::Value::String(token) => Some(token),
            _ => anyhow::bail!("unexpected socket token type"),
        };

        log::info!("connecting to {}...", options.socket_url);
        let mut socket = connect_ws(&options.socket_url)?;
        let manager = SqliteConnectionManager::file("sekshi.sqlite");
        let pool = r2d2::Pool::new(manager).unwrap();
        migrations::MIGRATIONS.to_latest(&mut pool.get().unwrap())?;

        if let Some(token) = socket_token {
            socket.write_message(Message::Text(token.to_string()))?;
        }

        let mut bot = Self {
            pool,
            client,
            socket,
            api_url: options.api_url,
            api_auth,
            handlers: vec![],
        };

        bot.add_handler(handlers::Emotes);
        bot.add_handler(handlers::Exit);
        bot.add_handler(handlers::SkipList::new(&now));
        bot.add_handler(handlers::HistorySkip);

        Ok(bot)
    }

    pub fn add_handler(&mut self, handler: impl Handler + Send + 'static) {
        self.handlers.push(Box::new(handler));
    }

    pub fn run(self) -> anyhow::Result<()> {
        let exit_flag = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&exit_flag))?;

        let (api_sender, api_receiver) = flume::bounded(10);
        let (received_message_sender, received_message_receiver) = flume::bounded(10);

        let pool = self.pool;
        let mut socket = self.socket;
        let mut handlers = self.handlers;
        let http_api = HttpApi::new(self.client, self.api_url, self.api_auth);

        let socket_exit_flag = Arc::clone(&exit_flag);
        let socket_thread = std::thread::spawn(move || {
            while !socket_exit_flag.load(Ordering::Relaxed) {
                // Process all queued messages.
                loop {
                    let message = socket.read_message();
                    let message = match message {
                        Ok(Message::Text(message)) => {
                            if message == "-" {
                                continue;
                            }
                            Some(message)
                        }
                        Ok(Message::Close(_)) => {
                            log::info!("connection ended");
                            break;
                        }
                        Ok(_) => None,
                        Err(tungstenite::Error::Io(io_err))
                            if io_err.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            break
                        }
                        Err(tungstenite::Error::ConnectionClosed) => {
                            log::info!("connection closed");
                            socket_exit_flag.store(true, Ordering::Relaxed);
                            return Ok(());
                        }
                        Err(err) => {
                            todo!("handle error {:?}", err)
                        }
                    };

                    if let Some(message) = message {
                        let message: handler::Message = serde_json::from_str(&message).unwrap();

                        if let Some(message_type) = message.into_message_type() {
                            let _ = received_message_sender.send(message_type);
                        }
                    }
                }

                match api_receiver.recv_timeout(Duration::from_millis(16)) {
                    Err(flume::RecvTimeoutError::Timeout) => continue,
                    Ok(handler::ApiMessage::SendChat(message)) => {
                        log::info!("sending chat message: {}", message);
                        let send_chat = serde_json::json!({
                            "command": "sendChat",
                            "data": message,
                        });
                        socket.write_message(Message::Text(send_chat.to_string()))?;
                    }
                    Ok(handler::ApiMessage::Exit) | Err(_) => {
                        log::info!("logging out");
                        let logout = serde_json::json!({ "command": "logout" });
                        socket.write_message(Message::Text(logout.to_string()))?;
                        socket.close(None)?;
                        break;
                    }
                }
            }

            anyhow::Result::<()>::Ok(())
        });

        let (handler_end_sender, end_receiver) = flume::bounded(1);
        let handler_exit_flag = Arc::clone(&exit_flag);
        let handler_thread = std::thread::spawn(move || {
            let mut retval = Ok(());

            'outer: while !handler_exit_flag.load(Ordering::Relaxed) {
                let message =
                    match received_message_receiver.recv_timeout(Duration::from_millis(16)) {
                        Ok(message) => message,
                        Err(flume::RecvTimeoutError::Timeout) => continue,
                        Err(err) => {
                            log::warn!("handler exiting because: {:?}", err);
                            break;
                        }
                    };

                // TODO spawn these onto a threadpool
                log::info!("handling message {:?}", message);
                let api = handler::Api::new(api_sender.clone(), pool.clone(), http_api.clone());
                for handler in handlers.iter_mut() {
                    match handler.handle(api.clone(), &message) {
                        Ok(..) => (),
                        Err(err) => {
                            // Exit if we are no longer authenticated so the bot can be restarted
                            if err.is::<UnauthorizedError>() {
                                api.exit();
                                retval = Err(err);
                                break 'outer;
                            }

                            api.send_message(format_args!("Could not handle message: {}", err));
                        }
                    }
                }
            }

            handler_end_sender.send(retval).unwrap();
        });

        let result = flume::Selector::new()
            .recv(&end_receiver, |err| err)
            .wait()?;

        socket_thread.join().unwrap()?;
        handler_thread.join().unwrap();

        result
    }
}
