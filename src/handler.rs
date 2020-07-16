use async_std::sync::Sender;
use serde::Deserialize;
use std::fmt::Display;

fn parse_message(input: &str) -> anyhow::Result<(&str, Vec<&str>)> {
    use nom::branch::alt;
    use nom::bytes::complete::{escaped, is_not, take_while};
    use nom::character::complete::{alpha1, char, space0, space1};
    use nom::combinator::{all_consuming, opt};
    use nom::multi::separated_list;
    use nom::sequence::{preceded, terminated, tuple};
    use nom::IResult;

    fn parser(input: &str) -> IResult<&str, (&str, Vec<&str>)> {
        let cmd_parser = preceded(char('!'), alpha1);
        let string_parser = escaped(is_not("\""), '\\', char('"'));
        let onearg_parser = alt((
            preceded(char('"'), terminated(string_parser, char('"'))),
            take_while(|c: char| !c.is_ascii_whitespace()),
        ));
        let args_parser = separated_list(space1, onearg_parser);
        let (input, (cmd, args, _trailing)) =
            tuple((cmd_parser, opt(preceded(space1, args_parser)), space0))(input)?;

        Ok((input, (cmd, args.unwrap_or_default())))
    }

    let full_parser = all_consuming(parser);

    let (_, result) = match full_parser(input) {
        Ok(result) => result,
        Err(nom::Err::Incomplete(_)) => anyhow::bail!("garbage data at end of message?"),
        Err(nom::Err::Error((_, kind))) => anyhow::bail!("parse error: {:?}", kind),
        Err(nom::Err::Failure((_, kind))) => anyhow::bail!("parse failure: {:?}", kind),
    };

    Ok(result)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    #[serde(rename = "userID")]
    pub user_id: String,
    pub message: String,
    #[serde(skip)]
    command: Option<ChatCommand>,
}

impl ChatMessage {
    fn parse(&mut self) {
        self.command = self.message.parse().ok();
    }

    pub fn command(&self) -> Option<&ChatCommand> {
        self.command.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct ChatCommand {
    pub command: String,
    pub arguments: Vec<String>,
}

impl std::str::FromStr for ChatCommand {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let (command, arguments) = parse_message(s)?;
        Ok(Self {
            command: command.to_string(),
            arguments: arguments.into_iter().map(ToOwned::to_owned).collect(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum MessageType {
    Authenticated,
    Guests { count: i64 },
    ChatMessage(ChatMessage),
    WaitlistUpdate { user_ids: Vec<String> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    command: String,
    #[serde(default)]
    data: serde_json::Value,
}

impl Message {
    pub fn into_message_type(self) -> Option<MessageType> {
        match self.command.as_str() {
            "authenticated" => Some(MessageType::Authenticated),
            "guests" => Some(MessageType::Guests {
                count: self.data.as_i64()?,
            }),
            "chatMessage" => {
                let mut chat_message: ChatMessage = serde_json::from_value(self.data).ok()?;
                chat_message.parse();
                Some(MessageType::ChatMessage(chat_message))
            }
            _ => None,
        }
    }
}

pub enum ApiMessage {
    Exit,
    SendChat(String),
}

#[derive(Clone)]
pub struct Api(Sender<ApiMessage>);
impl Api {
    pub fn new(sender: Sender<ApiMessage>) -> Self {
        Self(sender)
    }

    pub async fn send_message(&self, message: impl Display) {
        self.0.send(ApiMessage::SendChat(message.to_string())).await;
    }

    pub async fn exit(&self) {
        self.0.send(ApiMessage::Exit).await;
    }
}

#[async_trait::async_trait]
pub trait Handler: std::fmt::Debug {
    async fn handle(&mut self, bot: Api, message: &MessageType) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::parse_message;

    #[test]
    fn message_parser() -> anyhow::Result<()> {
        assert_eq!(parse_message("!e test")?, ("e", vec!["test"]),);
        assert_eq!(
            parse_message("!addemote \"test\" https://wlk.yt/assets/emoji/1f604.png")?,
            (
                "addemote",
                vec!["test", "https://wlk.yt/assets/emoji/1f604.png"]
            ),
        );
        Ok(())
    }
}
