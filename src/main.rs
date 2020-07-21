use gumdrop::{Options, ParsingStyle};
use sekshibot::{ConnectionOptions, SekshiBot};

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

fn main() -> anyhow::Result<()> {
    femme::with_level(log::LevelFilter::Info);
    let args = Cli::parse_args_or_exit(ParsingStyle::AllOptions);
    log::info!("args: {:?}", args);

    async_std::task::block_on(async move {
        let bot = SekshiBot::connect(ConnectionOptions {
            api_url: args.api_url,
            socket_url: args.socket_url,
            email: std::env::var("SEKSHIBOT_EMAIL").unwrap(),
            password: std::env::var("SEKSHIBOT_PASSWORD").unwrap(),
        })
        .await?;

        bot.run().await?;
        Ok(())
    })
}
