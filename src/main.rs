use anyhow::{bail, Result};
use gumdrop::{Options, ParsingStyle};
use sekshibot::{ConnectionOptions, SekshiBot, UnauthorizedError};

///
#[derive(Debug, Clone, Options)]
pub struct Cli {
    /// HTTP API endpoint of the üWave server to connect to.
    #[options(required)]
    pub api_url: String,
    /// WebSocket API endpoint of the üWave server to connect to.
    #[options(required)]
    pub socket_url: String,
    pub help: bool,
}

fn main() -> Result<()> {
    femme::with_level(log::LevelFilter::Info);
    let args = Cli::parse_args_or_exit(ParsingStyle::AllOptions);
    log::info!("args: {:?}", args);

    let email = match std::env::var("SEKSHIBOT_EMAIL") {
        Ok(email) => email,
        _ => bail!("missing SEKSHIBOT_EMAIL env var"),
    };
    let password = match std::env::var("SEKSHIBOT_PASSWORD") {
        Ok(password) => password,
        _ => bail!("missing SEKSHIBOT_PASSWORD env var"),
    };

    let result = async_std::task::block_on(async move {
        let bot = SekshiBot::connect(ConnectionOptions {
            api_url: args.api_url,
            socket_url: args.socket_url,
            email,
            password,
        })
        .await?;

        bot.run().await
    });

    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            if err.is::<UnauthorizedError>() {
                eprintln!("Error: {}", err);
                quit::with_code(75);
            } else {
                Err(err)
            }
        }
    }
}
