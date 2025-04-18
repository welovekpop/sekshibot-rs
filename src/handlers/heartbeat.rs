use crate::handler::{Api, Handler, MessageType};

#[derive(Debug, Default)]
pub struct Heartbeat;

impl Handler for Heartbeat {
    fn handle(&mut self, api: Api, message: &MessageType) -> anyhow::Result<()> {
        if !matches!(message, MessageType::Heartbeat) {
            return Ok(());
        }

        api.http.check_auth()?;

        Ok(())
    }
}
